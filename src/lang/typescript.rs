use super::{LanguageSupport, QueryKind};

pub struct TypeScriptSupport;

impl LanguageSupport for TypeScriptSupport {
    fn language(&self) -> tree_sitter::Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }

    fn extensions(&self) -> &[&str] {
        &["ts", "tsx"]
    }

    fn query_source(&self, kind: QueryKind) -> &str {
        match kind {
            QueryKind::Imports => r#"(import_statement source: (string) @source)"#,
            QueryKind::VariableDeclarations => {
                r#"
                (lexical_declaration
                    kind: "let"
                    (variable_declarator name: (identifier) @name)) @decl
                "#
            }
            QueryKind::ClassDeclarations => {
                r#"(class_declaration name: (type_identifier) @name body: (class_body) @body) @class"#
            }
            QueryKind::Exports => r#"(export_statement) @export"#,
            QueryKind::MemberAccesses => {
                r#"(member_expression object: (_) @object property: (property_identifier) @property) @access"#
            }
            QueryKind::Assignments => {
                r#"(assignment_expression left: (_) @left right: (_) @right) @assign"#
            }
            QueryKind::NewExpressions => {
                r#"(new_expression constructor: (_) @constructor) @new_expr"#
            }
            QueryKind::ConstructorParams => {
                r#"
                (method_definition
                    name: (property_identifier) @method_name
                    parameters: (formal_parameters) @params
                    (#eq? @method_name "constructor"))
                "#
            }
        }
    }
}

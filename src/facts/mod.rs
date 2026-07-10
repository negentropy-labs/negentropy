mod functions;
mod imports;
mod literals;
mod names;
mod state;

use std::path::Path;

use tree_sitter::Node;

#[cfg(test)]
use crate::parser::parse_source;
#[cfg(test)]
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct ImportEdge {
    pub raw_target: String,
    pub resolved_target: Option<String>,
    pub distance: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FunctionFact {
    pub name: String,
    pub line: usize,
    pub params: Vec<ParameterFact>,
    pub branches: Vec<BranchFact>,
    pub ead: f64,
    pub injected_interactions: usize,
    pub hardcoded_interactions: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ParameterFact {
    pub name: String,
    pub line: usize,
    pub type_hint: Option<String>,
    pub boolean_like: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct BranchFact {
    pub line: usize,
    pub condition: String,
    pub referenced_params: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CallFact {
    pub callee: String,
    pub line: usize,
    pub boolean_literal_args: Vec<BooleanArgumentFact>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct BooleanArgumentFact {
    pub index: usize,
    pub value: bool,
}

#[derive(Debug, Clone)]
pub struct MemberWrite {
    pub entity: String,
    pub line: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    Directory,
    File,
    Module,
    Function,
    Class,
    Type,
    Variable,
    Parameter,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NameFact {
    pub kind: NameKind,
    pub name: String,
    pub line: Option<usize>,
    pub exported: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralKind {
    String,
    Number,
    Boolean,
    Template,
    Regex,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LiteralFact {
    pub kind: LiteralKind,
    pub value: String,
    pub line: usize,
    pub parent_kind: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ModuleFacts {
    pub is_test: bool,
    pub is_entry_like: bool,
    pub is_generated_like: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FileFacts {
    pub module_id: String,
    pub module: ModuleFacts,
    pub names: Vec<NameFact>,
    pub literals: Vec<LiteralFact>,
    pub imports: Vec<ImportEdge>,
    pub export_complexity: f64,
    pub implementation_complexity: f64,
    pub functions: Vec<FunctionFact>,
    pub calls: Vec<CallFact>,
    pub mutable_declared: usize,
    pub mutable_mutated: usize,
    pub member_writes: Vec<MemberWrite>,
}

#[cfg(test)]
fn extract_facts(
    path: &Path,
    root: &Path,
    source: &str,
    extensions: &[String],
) -> Result<Option<FileFacts>> {
    let outcome = parse_source(path, root, source)?;
    let Some(tree) = outcome.tree else {
        return Ok(None);
    };

    Ok(Some(extract_facts_from_tree(
        path,
        root,
        source,
        tree.root_node(),
        extensions,
    )))
}

pub fn extract_facts_from_tree(
    path: &Path,
    root: &Path,
    source: &str,
    root_node: Node<'_>,
    extensions: &[String],
) -> FileFacts {
    let module_id = relative_module_id(path, root);
    let module = module_facts(path, root);
    let names = names::collect_name_facts(path, root, source, root_node);
    let literals = literals::collect_literal_facts(source, root_node);
    let imports = imports::collect_imports(path, root, source, root_node, extensions);
    let function_facts = functions::collect_function_facts(source, root_node);
    let state_facts = state::collect_state_facts(source, root_node);

    FileFacts {
        module_id,
        module,
        names,
        literals,
        imports,
        export_complexity: function_facts.export_complexity,
        implementation_complexity: function_facts.implementation_complexity,
        functions: function_facts.functions,
        calls: function_facts.calls,
        mutable_declared: state_facts.mutable_declared,
        mutable_mutated: state_facts.mutable_mutated,
        member_writes: state_facts.member_writes,
    }
}

fn relative_module_id(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn module_facts(path: &Path, root: &Path) -> ModuleFacts {
    let module_id = relative_module_id(path, root);
    let lower = module_id.to_ascii_lowercase();
    let file_stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    ModuleFacts {
        is_test: lower.contains("__tests__/")
            || lower.contains("/tests/")
            || lower.contains(".test.")
            || lower.contains(".spec.")
            || lower.ends_with("_test.ts")
            || lower.ends_with("_test.js"),
        is_entry_like: matches!(
            file_stem.as_str(),
            "index" | "main" | "app" | "server" | "cli"
        ),
        is_generated_like: lower.contains("/generated/")
            || lower.contains(".generated.")
            || lower.contains("/gen/")
            || lower.ends_with(".gen.ts")
            || lower.ends_with(".gen.js"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{LiteralKind, NameKind, extract_facts};

    #[test]
    fn extracts_language_neutral_facts_from_ts_source() {
        let source = r#"
export function save(user: User, dryRun: boolean) {
  if (dryRun) {
    return "skipped";
  }
  return format(user.id, true);
}

const status = "active";
"#;
        let path = Path::new("/repo/src/service.test.ts");
        let root = Path::new("/repo");
        let facts = extract_facts(path, root, source, &[".ts".to_string()])
            .expect("extract facts")
            .expect("file facts");

        assert!(facts.module.is_test);
        assert!(facts.names.iter().any(|name| {
            name.kind == NameKind::Function && name.name == "save" && name.exported
        }));
        assert!(
            facts
                .names
                .iter()
                .any(|name| { name.kind == NameKind::Variable && name.name == "status" })
        );
        assert!(
            facts.literals.iter().any(|literal| {
                literal.kind == LiteralKind::String && literal.value == "active"
            })
        );

        let save = facts
            .functions
            .iter()
            .find(|function| function.name == "save")
            .expect("save function");
        assert!(
            save.params
                .iter()
                .any(|param| param.name == "dryRun" && param.boolean_like)
        );
        assert!(save.branches.iter().any(|branch| {
            branch
                .referenced_params
                .iter()
                .any(|param| param == "dryRun")
        }));
        assert!(facts.calls.iter().any(|call| {
            call.callee == "format"
                && call
                    .boolean_literal_args
                    .iter()
                    .any(|arg| arg.index == 1 && arg.value)
        }));
    }
}

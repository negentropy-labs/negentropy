use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use crate::parser::{node_text, parse_source, walk_named_nodes};

#[derive(Debug, Clone)]
pub struct ImportEdge {
    pub raw_target: String,
    pub resolved_target: Option<String>,
    pub distance: usize,
}

#[derive(Debug, Clone)]
pub struct FunctionMetric {
    pub name: String,
    pub line: usize,
    pub ead: f64,
    pub injected_interactions: usize,
    pub hardcoded_interactions: usize,
}

#[derive(Debug, Clone)]
pub struct MemberWrite {
    pub entity: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct FileFacts {
    pub module_id: String,
    pub imports: Vec<ImportEdge>,
    pub export_complexity: f64,
    pub implementation_complexity: f64,
    pub functions: Vec<FunctionMetric>,
    pub mutable_declared: usize,
    pub mutable_mutated: usize,
    pub member_writes: Vec<MemberWrite>,
}

pub fn extract_facts(
    path: &Path,
    root: &Path,
    source: &str,
    extensions: &[String],
) -> Result<Option<FileFacts>> {
    let tree = parse_source(source)?;
    let root_node = tree.root_node();

    let module_id = relative_module_id(path, root);

    let mut imports = Vec::new();
    let mut export_complexity = 0.0f64;
    let mut implementation_complexity = 0.0f64;
    let mut functions = Vec::new();

    let mut mutable_declared = 0usize;
    let mut mutable_names = HashSet::new();
    let mut mutated_names = HashSet::new();

    let mut member_writes = Vec::new();

    walk_named_nodes(root_node, |node| {
        let kind = node.kind();

        if kind == "import_statement" || kind == "export_statement" {
            if let Some(raw_target) = extract_string_child(node, source)
                && (raw_target.starts_with("./") || raw_target.starts_with("../"))
            {
                imports.push(ImportEdge {
                    distance: relative_distance(&raw_target),
                    resolved_target: resolve_import_target(path, root, &raw_target, extensions),
                    raw_target,
                });
            }
        }

        if kind.starts_with("export") {
            export_complexity += 1.0;
            if let Some(func) = first_function_child(node) {
                export_complexity += parameter_names(func, source).len() as f64;
            }
        }

        if kind.ends_with("_statement") && kind != "import_statement" && kind != "export_statement"
        {
            implementation_complexity += 1.0;
        }

        if is_function_like(kind)
            && let Some(metric) = function_metric(node, source)
        {
            functions.push(metric);
        }

        if is_mutable_declaration(node, source) {
            for name in declared_identifiers(node, source) {
                mutable_declared += 1;
                mutable_names.insert(name);
            }
        }

        if kind == "assignment_expression" {
            if let Some(left) = node.child_by_field_name("left") {
                if left.kind() == "identifier" {
                    if let Some(name) = node_text(left, source)
                        && mutable_names.contains(name)
                    {
                        mutated_names.insert(name.to_string());
                    }
                } else if left.kind() == "member_expression"
                    && let Some(entity) = member_entity(left, source)
                {
                    member_writes.push(MemberWrite {
                        entity,
                        line: left.start_position().row + 1,
                    });
                }
            }
        }

        if kind == "update_expression"
            && let Some(arg) = node.child_by_field_name("argument")
        {
            if arg.kind() == "identifier" {
                if let Some(name) = node_text(arg, source)
                    && mutable_names.contains(name)
                {
                    mutated_names.insert(name.to_string());
                }
            } else if arg.kind() == "member_expression"
                && let Some(entity) = member_entity(arg, source)
            {
                member_writes.push(MemberWrite {
                    entity,
                    line: arg.start_position().row + 1,
                });
            }
        }
    });

    if implementation_complexity == 0.0 {
        implementation_complexity = 1.0;
    }

    Ok(Some(FileFacts {
        module_id,
        imports,
        export_complexity,
        implementation_complexity,
        functions,
        mutable_declared,
        mutable_mutated: mutated_names.len(),
        member_writes,
    }))
}

fn relative_module_id(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn extract_string_child(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "string" {
            let text = node_text(child, source)?;
            return Some(unquote(text));
        }
    }
    None
}

fn unquote(s: &str) -> String {
    s.trim_matches(['\"', '\'', '`']).to_string()
}

fn relative_distance(path: &str) -> usize {
    path.split('/').filter(|seg| *seg == "..").count()
}

fn resolve_import_target(
    path: &Path,
    root: &Path,
    import: &str,
    extensions: &[String],
) -> Option<String> {
    let parent = path.parent()?;
    let raw = parent.join(import);

    let mut candidates = Vec::new();

    if raw.extension().is_some() {
        candidates.push(raw.clone());
    } else {
        for ext in extensions {
            let suffix = ext.trim_start_matches('.');
            candidates.push(raw.with_extension(suffix));
        }
        for ext in extensions {
            let suffix = ext.trim_start_matches('.');
            candidates.push(raw.join("index").with_extension(suffix));
        }
    }

    for candidate in candidates {
        if candidate.exists() {
            let rel = candidate
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            return Some(rel);
        }
    }

    None
}

fn is_function_like(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration" | "function_expression" | "arrow_function" | "method_definition"
    )
}

fn function_metric(node: Node<'_>, source: &str) -> Option<FunctionMetric> {
    let line = node.start_position().row + 1;
    let name = function_name(node, source).unwrap_or_else(|| format!("anonymous@{line}"));

    let params = parameter_names(node, source);
    let body = node.child_by_field_name("body")?;

    let mut external_reads = 0usize;
    let mut self_reads = 0usize;
    let mut injected = 0usize;
    let mut hardcoded = 0usize;

    walk_named_nodes(body, |child| {
        let kind = child.kind();
        if kind == "member_expression"
            && let Some(object) = child.child_by_field_name("object")
        {
            if object.kind() == "this" {
                self_reads += 1;
            } else if object.kind() == "identifier"
                && let Some(obj_name) = node_text(object, source)
                && params.contains(obj_name)
            {
                external_reads += 1;
                injected += 1;
            }
        }

        if kind == "new_expression" {
            hardcoded += 1;
        }
    });

    let ead = (external_reads as i64 - self_reads as i64).max(0) as f64;

    Some(FunctionMetric {
        name,
        line,
        ead,
        injected_interactions: injected,
        hardcoded_interactions: hardcoded,
    })
}

fn function_name(node: Node<'_>, source: &str) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name("name") {
        return node_text(name_node, source).map(ToString::to_string);
    }

    if node.kind() == "method_definition"
        && let Some(name_node) = node.child_by_field_name("name")
    {
        return node_text(name_node, source).map(ToString::to_string);
    }

    None
}

fn parameter_names(node: Node<'_>, source: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    if let Some(params_node) = node.child_by_field_name("parameters") {
        walk_named_nodes(params_node, |child| {
            if child.kind() == "identifier"
                && let Some(name) = node_text(child, source)
            {
                names.insert(name.to_string());
            }
        });
    }
    names
}

fn first_function_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| is_function_like(child.kind()))
}

fn is_mutable_declaration(node: Node<'_>, source: &str) -> bool {
    if node.kind() == "lexical_declaration"
        && let Some(text) = node_text(node, source)
    {
        return text.trim_start().starts_with("let ");
    }

    node.kind() == "variable_declaration"
}

fn declared_identifiers(node: Node<'_>, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = node.walk();
    for decl in node.named_children(&mut cursor) {
        if decl.kind() == "variable_declarator"
            && let Some(name_node) = decl.child_by_field_name("name")
            && name_node.kind() == "identifier"
            && let Some(name) = node_text(name_node, source)
        {
            names.push(name.to_string());
        }
    }
    names
}

fn member_entity(member: Node<'_>, source: &str) -> Option<String> {
    let object = member.child_by_field_name("object")?;
    let property = member.child_by_field_name("property")?;

    let obj = if object.kind() == "this" {
        "this".to_string()
    } else {
        node_text(object, source)?.to_string()
    };

    let prop = node_text(property, source)?.to_string();
    Some(format!("{obj}.{prop}"))
}

#[allow(dead_code)]
fn _join(root: &Path, rel: &str) -> PathBuf {
    root.join(rel)
}

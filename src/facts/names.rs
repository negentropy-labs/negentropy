use std::path::{Component, Path};

use tree_sitter::Node;

use crate::parser::{node_text, walk_named_nodes};

use super::{NameFact, NameKind};

pub(super) fn collect_name_facts(
    path: &Path,
    root: &Path,
    source: &str,
    root_node: Node<'_>,
) -> Vec<NameFact> {
    let mut names = path_name_facts(path, root);

    walk_named_nodes(root_node, |node| {
        let kind = node.kind();
        let exported = has_export_ancestor(node);

        if is_function_like(kind) {
            if let Some(name) = named_child_text(node, source) {
                names.push(NameFact {
                    kind: NameKind::Function,
                    name,
                    line: Some(node.start_position().row + 1),
                    exported,
                });
            }

            names.extend(parameter_name_facts(node, source));
        } else if kind == "class_declaration" {
            if let Some(name) = named_child_text(node, source) {
                names.push(NameFact {
                    kind: NameKind::Class,
                    name,
                    line: Some(node.start_position().row + 1),
                    exported,
                });
            }
        } else if kind == "type_alias_declaration" || kind == "interface_declaration" {
            if let Some(name) = named_child_text(node, source) {
                names.push(NameFact {
                    kind: NameKind::Type,
                    name,
                    line: Some(node.start_position().row + 1),
                    exported,
                });
            }
        } else if kind == "variable_declarator"
            && let Some(name_node) = node.child_by_field_name("name")
            && name_node.kind() == "identifier"
            && let Some(name) = node_text(name_node, source)
        {
            names.push(NameFact {
                kind: NameKind::Variable,
                name: name.to_string(),
                line: Some(name_node.start_position().row + 1),
                exported,
            });
        }
    });

    names
}

fn path_name_facts(path: &Path, root: &Path) -> Vec<NameFact> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let mut facts = Vec::new();

    facts.push(NameFact {
        kind: NameKind::Module,
        name: rel.to_string_lossy().replace('\\', "/"),
        line: None,
        exported: false,
    });

    if let Some(parent) = rel.parent() {
        for component in parent.components() {
            let Component::Normal(part) = component else {
                continue;
            };
            let Some(part) = part.to_str() else {
                continue;
            };
            facts.push(NameFact {
                kind: NameKind::Directory,
                name: part.to_string(),
                line: None,
                exported: false,
            });
        }
    }

    if let Some(stem) = rel.file_stem().and_then(|stem| stem.to_str()) {
        facts.push(NameFact {
            kind: NameKind::File,
            name: stem.to_string(),
            line: None,
            exported: false,
        });
    }

    facts
}

fn parameter_name_facts(node: Node<'_>, source: &str) -> Vec<NameFact> {
    let Some(params_node) = node.child_by_field_name("parameters") else {
        return Vec::new();
    };
    let mut facts = Vec::new();

    walk_named_nodes(params_node, |child| {
        if child.kind() == "identifier"
            && !is_type_position(child)
            && let Some(name) = node_text(child, source)
        {
            facts.push(NameFact {
                kind: NameKind::Parameter,
                name: name.to_string(),
                line: Some(child.start_position().row + 1),
                exported: false,
            });
        }
    });

    facts
}

fn named_child_text(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|name| node_text(name, source))
        .map(ToString::to_string)
}

fn is_function_like(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration" | "function_expression" | "arrow_function" | "method_definition"
    )
}

fn has_export_ancestor(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind().starts_with("export") {
            return true;
        }
        if parent.kind() == "program" {
            return false;
        }
        node = parent;
    }

    false
}

fn is_type_position(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        let kind = parent.kind();
        if kind.contains("type") || kind == "type_annotation" {
            return true;
        }
        if kind == "formal_parameters" || kind == "parameters" {
            return false;
        }
        node = parent;
    }

    false
}

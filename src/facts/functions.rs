use std::collections::HashSet;

use tree_sitter::Node;

use crate::parser::{node_text, walk_named_nodes};

use super::FunctionMetric;

pub(super) struct FunctionFacts {
    pub export_complexity: f64,
    pub implementation_complexity: f64,
    pub functions: Vec<FunctionMetric>,
}

pub(super) fn collect_function_facts(source: &str, root_node: Node<'_>) -> FunctionFacts {
    let mut export_complexity = 0.0f64;
    let mut implementation_complexity = 0.0f64;
    let mut functions = Vec::new();

    walk_named_nodes(root_node, |node| {
        let kind = node.kind();

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
    });

    if implementation_complexity == 0.0 {
        implementation_complexity = 1.0;
    }

    FunctionFacts {
        export_complexity,
        implementation_complexity,
        functions,
    }
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

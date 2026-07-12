use std::collections::HashSet;

use tree_sitter::Node;

use crate::parser::{node_text, walk_named_nodes};

use super::{
    BooleanArgumentFact, BooleanEvidence, BranchFact, CallFact, FunctionFact, ParameterFact,
};

pub(super) struct FunctionFacts {
    pub export_complexity: f64,
    pub implementation_complexity: f64,
    pub functions: Vec<FunctionFact>,
    pub calls: Vec<CallFact>,
}

pub(super) fn collect_function_facts(source: &str, root_node: Node<'_>) -> FunctionFacts {
    let mut export_complexity = 0.0f64;
    let mut implementation_complexity = 0.0f64;
    let mut functions = Vec::new();
    let mut calls = Vec::new();

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

        if kind == "call_expression"
            && let Some(call) = call_fact(node, source)
        {
            calls.push(call);
        }
    });

    if implementation_complexity == 0.0 {
        implementation_complexity = 1.0;
    }

    FunctionFacts {
        export_complexity,
        implementation_complexity,
        functions,
        calls,
    }
}

fn is_function_like(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration" | "function_expression" | "arrow_function" | "method_definition"
    )
}

fn function_metric(node: Node<'_>, source: &str) -> Option<FunctionFact> {
    let line = node.start_position().row + 1;
    let name = function_name(node, source).unwrap_or_else(|| format!("anonymous@{line}"));

    let params = parameter_facts(node, source);
    let param_names = params
        .iter()
        .map(|param| param.name.clone())
        .collect::<HashSet<_>>();
    let body = node.child_by_field_name("body")?;

    let mut branches = Vec::new();
    let mut external_reads = 0usize;
    let mut self_reads = 0usize;
    let mut injected = 0usize;
    let mut hardcoded = 0usize;

    walk_named_nodes(body, |child| {
        let kind = child.kind();
        if is_branch_like(kind)
            && let Some(branch) = branch_fact(child, source, &param_names)
        {
            branches.push(branch);
        }

        if kind == "member_expression"
            && let Some(object) = child.child_by_field_name("object")
        {
            if object.kind() == "this" {
                self_reads += 1;
            } else if object.kind() == "identifier"
                && let Some(obj_name) = node_text(object, source)
                && param_names.contains(obj_name)
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

    Some(FunctionFact {
        name,
        line,
        params,
        branches,
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
    parameter_facts(node, source)
        .into_iter()
        .map(|param| param.name)
        .collect()
}

fn parameter_facts(node: Node<'_>, source: &str) -> Vec<ParameterFact> {
    let mut params = Vec::new();
    let mut seen = HashSet::new();
    if let Some(params_node) = node.child_by_field_name("parameters") {
        walk_named_nodes(params_node, |child| {
            if child.kind() == "identifier"
                && !is_type_position(child)
                && let Some(name) = node_text(child, source)
                && seen.insert(name.to_string())
            {
                let type_hint = parameter_type_hint(child, source);
                let boolean_evidence = boolean_evidence(name, child, source, type_hint.as_deref());
                params.push(ParameterFact {
                    name: name.to_string(),
                    line: child.start_position().row + 1,
                    boolean_evidence,
                    type_hint,
                });
            }
        });
    }
    params
}

fn boolean_evidence(
    name: &str,
    node: Node<'_>,
    source: &str,
    type_hint: Option<&str>,
) -> Option<BooleanEvidence> {
    if type_hint.is_some_and(is_boolean_type_hint) {
        Some(BooleanEvidence::ExplicitType)
    } else if has_boolean_default(node, source) {
        Some(BooleanEvidence::LiteralDefault)
    } else if is_boolean_like_name(name) {
        Some(BooleanEvidence::NameHeuristic)
    } else {
        None
    }
}

fn first_function_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| is_function_like(child.kind()))
}

fn branch_fact(node: Node<'_>, source: &str, params: &HashSet<String>) -> Option<BranchFact> {
    let condition = node.child_by_field_name("condition")?;
    let mut referenced_params = HashSet::new();

    walk_named_nodes(condition, |child| {
        if child.kind() == "identifier"
            && let Some(name) = node_text(child, source)
            && params.contains(name)
        {
            referenced_params.insert(name.to_string());
        }
    });

    if referenced_params.is_empty() {
        return None;
    }

    let mut referenced_params = referenced_params.into_iter().collect::<Vec<_>>();
    referenced_params.sort();

    Some(BranchFact {
        line: node.start_position().row + 1,
        condition: node_text(condition, source).unwrap_or_default().to_string(),
        referenced_params,
    })
}

fn call_fact(node: Node<'_>, source: &str) -> Option<CallFact> {
    let callee = node
        .child_by_field_name("function")
        .and_then(|function| node_text(function, source))
        .unwrap_or("anonymous")
        .to_string();
    let args = node.child_by_field_name("arguments")?;
    let mut boolean_literal_args = Vec::new();

    let mut cursor = args.walk();
    for (index, arg) in args.named_children(&mut cursor).enumerate() {
        match arg.kind() {
            "true" => boolean_literal_args.push(BooleanArgumentFact { index, value: true }),
            "false" => boolean_literal_args.push(BooleanArgumentFact {
                index,
                value: false,
            }),
            _ => {}
        }
    }

    if boolean_literal_args.is_empty() {
        return None;
    }

    Some(CallFact {
        callee,
        line: node.start_position().row + 1,
        boolean_literal_args,
    })
}

fn parameter_type_hint(node: Node<'_>, source: &str) -> Option<String> {
    let mut current = node;
    while let Some(parent) = current.parent() {
        let kind = parent.kind();
        if kind == "formal_parameters" || kind == "parameters" {
            return None;
        }
        if let Some(text) = node_text(parent, source)
            && let Some((_, hint)) = text.split_once(':')
        {
            return Some(clean_type_hint(hint));
        }
        current = parent;
    }

    None
}

fn clean_type_hint(hint: &str) -> String {
    let hint = hint.split_once('=').map_or(hint, |(hint, _)| hint);
    hint.trim()
        .trim_end_matches(',')
        .trim_end_matches('?')
        .trim()
        .to_string()
}

fn has_boolean_default(node: Node<'_>, source: &str) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        let kind = parent.kind();
        if kind == "formal_parameters" || kind == "parameters" {
            return false;
        }

        if let Some(text) = node_text(parent, source)
            && let Some((_, default)) = text.split_once('=')
            && starts_with_boolean_literal(default.trim())
        {
            return true;
        }

        current = parent;
    }

    false
}

fn starts_with_boolean_literal(value: &str) -> bool {
    ["true", "false"].iter().any(|literal| {
        value == *literal
            || value.strip_prefix(literal).is_some_and(|rest| {
                rest.chars()
                    .next()
                    .is_none_or(|ch| !ch.is_ascii_alphanumeric())
            })
    })
}

fn is_branch_like(kind: &str) -> bool {
    matches!(
        kind,
        "if_statement"
            | "while_statement"
            | "do_statement"
            | "for_statement"
            | "ternary_expression"
    )
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

fn is_boolean_like_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    has_boolean_prefix(name, "is")
        || has_boolean_prefix(name, "has")
        || has_boolean_prefix(name, "can")
        || has_boolean_prefix(name, "should")
        || has_boolean_prefix(name, "enable")
        || has_boolean_prefix(name, "disable")
        || has_boolean_prefix(name, "include")
        || has_boolean_prefix(name, "exclude")
        || has_boolean_prefix(name, "allow")
        || has_boolean_prefix(name, "skip")
        || matches!(
            lower.as_str(),
            "dryrun" | "force" | "strict" | "verbose" | "debug" | "flag"
        )
}

fn has_boolean_prefix(name: &str, prefix: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower == prefix {
        return true;
    }

    if !lower.starts_with(prefix) {
        return false;
    }

    let rest = &name[prefix.len()..];
    rest.starts_with('_')
        || rest.starts_with('-')
        || rest
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn is_boolean_type_hint(hint: &str) -> bool {
    let lower = hint.to_ascii_lowercase();
    lower == "boolean" || lower == "bool"
}

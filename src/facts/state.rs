use std::collections::HashSet;

use tree_sitter::Node;

use crate::parser::{node_text, walk_named_nodes};

use super::MemberWrite;

pub(super) struct StateFacts {
    pub mutable_declared: usize,
    pub mutable_mutated: usize,
    pub member_writes: Vec<MemberWrite>,
}

pub(super) fn collect_state_facts(source: &str, root_node: Node<'_>) -> StateFacts {
    let mut mutable_declared = 0usize;
    let mut mutable_names = HashSet::new();
    let mut mutated_names = HashSet::new();
    let mut member_writes = Vec::new();

    walk_named_nodes(root_node, |node| {
        if is_mutable_declaration(node, source) {
            for name in declared_identifiers(node, source) {
                mutable_declared += 1;
                mutable_names.insert(name);
            }
        }

        if node.kind() == "assignment_expression"
            && let Some(left) = node.child_by_field_name("left")
        {
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

        if node.kind() == "update_expression"
            && let Some(argument) = node.child_by_field_name("argument")
        {
            if argument.kind() == "identifier" {
                if let Some(name) = node_text(argument, source)
                    && mutable_names.contains(name)
                {
                    mutated_names.insert(name.to_string());
                }
            } else if argument.kind() == "member_expression"
                && let Some(entity) = member_entity(argument, source)
            {
                member_writes.push(MemberWrite {
                    entity,
                    line: argument.start_position().row + 1,
                });
            }
        }
    });

    StateFacts {
        mutable_declared,
        mutable_mutated: mutated_names.len(),
        member_writes,
    }
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

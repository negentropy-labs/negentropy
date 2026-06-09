use tree_sitter::Node;

use crate::parser::{node_text, walk_named_nodes};

use super::{LiteralFact, LiteralKind};

pub(super) fn collect_literal_facts(source: &str, root_node: Node<'_>) -> Vec<LiteralFact> {
    let mut literals = Vec::new();

    walk_named_nodes(root_node, |node| {
        let Some(kind) = literal_kind(node.kind()) else {
            return;
        };
        let Some(text) = node_text(node, source) else {
            return;
        };

        literals.push(LiteralFact {
            kind,
            value: normalize_literal(text, kind),
            line: node.start_position().row + 1,
            parent_kind: node.parent().map(|parent| parent.kind().to_string()),
        });
    });

    literals
}

fn literal_kind(kind: &str) -> Option<LiteralKind> {
    match kind {
        "string" => Some(LiteralKind::String),
        "number" => Some(LiteralKind::Number),
        "true" | "false" => Some(LiteralKind::Boolean),
        "template_string" => Some(LiteralKind::Template),
        "regex" => Some(LiteralKind::Regex),
        _ => None,
    }
}

fn normalize_literal(text: &str, kind: LiteralKind) -> String {
    match kind {
        LiteralKind::String => text.trim_matches(['"', '\'', '`']).to_string(),
        _ => text.to_string(),
    }
}

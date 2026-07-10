use std::path::Path;

use tree_sitter::Node;

use crate::parser::{node_text, walk_named_nodes};
use crate::resolver::ModuleResolver;

use super::ImportEdge;

pub(super) fn collect_imports(
    path: &Path,
    source: &str,
    root_node: Node<'_>,
    resolver: &ModuleResolver,
) -> Vec<ImportEdge> {
    let mut imports = Vec::new();

    walk_named_nodes(root_node, |node| {
        let kind = node.kind();
        if (kind == "import_statement" || kind == "export_statement")
            && let Some(raw_target) = extract_string_child(node, source)
        {
            let resolved = resolver.resolve(path, &raw_target);
            if !resolved.is_internal_candidate {
                return;
            }

            imports.push(ImportEdge {
                distance: relative_distance(&raw_target),
                resolved_target: resolved.resolved_target,
                raw_target,
            });
        }
    });

    imports
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
    s.trim_matches(['"', '\'', '`']).to_string()
}

fn relative_distance(path: &str) -> usize {
    path.split('/').filter(|seg| *seg == "..").count()
}

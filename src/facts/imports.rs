use std::path::{Path, PathBuf};

use tree_sitter::Node;

use crate::parser::{node_text, walk_named_nodes};

use super::ImportEdge;

pub(super) fn collect_imports(
    path: &Path,
    root: &Path,
    source: &str,
    root_node: Node<'_>,
    extensions: &[String],
) -> Vec<ImportEdge> {
    let mut imports = Vec::new();

    walk_named_nodes(root_node, |node| {
        let kind = node.kind();
        if (kind == "import_statement" || kind == "export_statement")
            && let Some(raw_target) = extract_string_child(node, source)
            && (raw_target.starts_with("./") || raw_target.starts_with("../"))
        {
            imports.push(ImportEdge {
                distance: relative_distance(&raw_target),
                resolved_target: resolve_import_target(path, root, &raw_target, extensions),
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

#[allow(dead_code)]
fn _join(root: &Path, rel: &str) -> PathBuf {
    root.join(rel)
}

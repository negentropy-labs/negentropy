use tree_sitter::{Query, QueryCursor};

use crate::lang::ParsedFile;
use crate::report::{Diagnostic, Severity};

use super::Metric;

/// Interface-Implementation Entropy (IIE)
/// Measures the ratio of public surface area to internal implementation.
/// IIE = public_signature_nodes / internal_body_nodes
/// IIE > 1 means the module is "shallow" — more interface than substance.
pub struct IieMetric {
    pub export_query: Query,
}

impl Metric for IieMetric {
    fn id(&self) -> &str {
        "iie"
    }

    fn analyze_file(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let source = &file.source;
        let mut cursor = QueryCursor::new();
        let export_idx = self
            .export_query
            .capture_index_for_name("export")
            .unwrap();
        let matches = cursor.matches(
            &self.export_query,
            file.tree.root_node(),
            source.as_slice(),
        );

        let mut diagnostics = Vec::new();

        for m in matches {
            for cap in m.captures {
                if cap.index != export_idx {
                    continue;
                }

                let export_node = cap.node;
                // Count signature nodes (direct children of export) vs body nodes
                let (signature_nodes, body_nodes) =
                    count_surface_vs_body(export_node);

                if body_nodes == 0 {
                    continue; // type exports, interfaces — skip
                }

                let iie = signature_nodes as f64 / body_nodes as f64;
                if iie > 1.0 {
                    let line = export_node.start_position().row + 1;
                    let snippet = export_node
                        .utf8_text(source)
                        .unwrap_or("")
                        .lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(60)
                        .collect::<String>();
                    diagnostics.push(Diagnostic {
                        id: format!("IIE-{:03}", diagnostics.len() + 1),
                        severity: if iie > 2.0 {
                            Severity::Warning
                        } else {
                            Severity::Info
                        },
                        metric: "iie".into(),
                        file: file.path.display().to_string(),
                        line,
                        message: format!(
                            "Shallow module (IIE = {iie:.2}): interface ({signature_nodes} nodes) > implementation ({body_nodes} nodes). `{snippet}...`"
                        ),
                        suggestion: "This export may be a trivial wrapper. Consider inlining or deepening its implementation".into(),
                    });
                }
            }
        }

        diagnostics
    }
}

/// Count signature surface nodes vs body implementation nodes.
/// Signature = parameters, return types, decorators.
/// Body = statements inside function/method bodies.
fn count_surface_vs_body(node: tree_sitter::Node) -> (usize, usize) {
    let mut signature = 0usize;
    let mut body = 0usize;

    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        let kind = current.kind();
        match kind {
            "formal_parameters" | "type_annotation" | "type_parameters"
            | "accessibility_modifier" | "decorator" => {
                signature += count_descendants(current);
            }
            "statement_block" => {
                body += count_descendants(current);
            }
            _ => {
                let mut child_cursor = current.walk();
                for child in current.children(&mut child_cursor) {
                    stack.push(child);
                }
            }
        }
    }

    (signature, body)
}

fn count_descendants(node: tree_sitter::Node) -> usize {
    let mut count = 1;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count += count_descendants(child);
    }
    count
}

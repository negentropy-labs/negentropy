use tree_sitter::{Query, QueryCursor};

use crate::lang::ParsedFile;
use crate::report::{Diagnostic, Severity};

use super::Metric;

/// External Attribute Dependency (EAD)
/// Detects Feature Envy: methods that access external object properties
/// more than their own (this.) properties.
/// EAD = max(0, N_external - N_self)
pub struct EadMetric {
    pub member_query: Query,
    pub class_query: Query,
}

impl Metric for EadMetric {
    fn id(&self) -> &str {
        "ead"
    }

    fn analyze_file(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let source = &file.source;

        // Find all class methods
        let mut diagnostics = Vec::new();
        let mut class_cursor = QueryCursor::new();
        let class_body_idx = self.class_query.capture_index_for_name("body").unwrap();
        let class_name_idx = self.class_query.capture_index_for_name("name").unwrap();

        let matches = class_cursor.matches(
            &self.class_query,
            file.tree.root_node(),
            source.as_slice(),
        );

        for m in matches {
            let class_name = m.captures.iter()
                .find(|c| c.index == class_name_idx)
                .and_then(|c| c.node.utf8_text(source).ok())
                .unwrap_or("?");

            let body_node = match m.captures.iter().find(|c| c.index == class_body_idx) {
                Some(c) => c.node,
                None => continue,
            };

            // Iterate over methods in class body
            let mut walk = body_node.walk();
            for child in body_node.children(&mut walk) {
                if child.kind() != "method_definition" {
                    continue;
                }

                let method_name = child
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source).ok())
                    .unwrap_or("?");

                if method_name == "constructor" {
                    continue;
                }

                let (self_accesses, external_accesses) =
                    count_member_accesses(child, source);

                let ead = external_accesses.saturating_sub(self_accesses);

                if ead > 2 {
                    let line = child.start_position().row + 1;
                    diagnostics.push(Diagnostic {
                        id: format!("EAD-{:03}", diagnostics.len() + 1),
                        severity: if ead > 5 {
                            Severity::Warning
                        } else {
                            Severity::Info
                        },
                        metric: "ead".into(),
                        file: file.path.display().to_string(),
                        line,
                        message: format!(
                            "Feature Envy in `{class_name}.{method_name}()`: {external_accesses} external vs {self_accesses} self property accesses (EAD = {ead})"
                        ),
                        suggestion: format!("Consider moving this method to the class whose data it primarily uses"),
                    });
                }
            }
        }

        diagnostics
    }
}

/// Count `this.xxx` (self) vs `param.xxx` (external) member accesses within a method node.
fn count_member_accesses(method_node: tree_sitter::Node, _source: &[u8]) -> (usize, usize) {
    let mut self_count = 0;
    let mut external_count = 0;

    let mut stack = vec![method_node];
    while let Some(node) = stack.pop() {
        if node.kind() == "member_expression" {
            if let Some(object) = node.child_by_field_name("object") {
                if object.kind() == "this" {
                    self_count += 1;
                } else if object.kind() == "identifier" {
                    external_count += 1;
                }
            }
            // Don't recurse into nested member_expression to avoid double-counting
            continue;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    (self_count, external_count)
}

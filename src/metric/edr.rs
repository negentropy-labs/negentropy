use tree_sitter::{Query, QueryCursor};

use crate::lang::ParsedFile;
use crate::report::{Diagnostic, Severity};

use super::Metric;

/// Explicit Dependency Rate (EDR)
/// Measures the ratio of injected (constructor) dependencies vs hardcoded (new) dependencies.
/// EDR = injected / (injected + hardcoded)
/// High EDR = good testability (more seams for mocking).
pub struct EdrMetric {
    pub constructor_query: Query,
    pub new_query: Query,
}

impl Metric for EdrMetric {
    fn id(&self) -> &str {
        "edr"
    }

    fn analyze_file(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let source = &file.source;

        // Count constructor parameters (injected dependencies)
        let injected = {
            let mut cursor = QueryCursor::new();
            let params_idx = self
                .constructor_query
                .capture_index_for_name("params")
                .unwrap();
            let mut count = 0usize;
            let matches = cursor.matches(
                &self.constructor_query,
                file.tree.root_node(),
                source.as_slice(),
            );
            for m in matches {
                for cap in m.captures {
                    if cap.index == params_idx {
                        // Count the number of formal parameters (children of type required_parameter or optional_parameter)
                        let mut child_cursor = cap.node.walk();
                        for child in cap.node.children(&mut child_cursor) {
                            let kind = child.kind();
                            if kind == "required_parameter" || kind == "optional_parameter" {
                                count += 1;
                            }
                        }
                    }
                }
            }
            count
        };

        // Count `new` expressions (hardcoded dependencies)
        let hardcoded = {
            let mut cursor = QueryCursor::new();
            let new_idx = self
                .new_query
                .capture_index_for_name("new_expr")
                .unwrap();
            cursor
                .matches(&self.new_query, file.tree.root_node(), source.as_slice())
                .flat_map(|m| m.captures.iter())
                .filter(|c| c.index == new_idx)
                .count()
        };

        let total = injected + hardcoded;
        if total == 0 {
            return Vec::new();
        }

        let edr = injected as f64 / total as f64;

        if edr < 0.5 && hardcoded > 0 {
            let mut diagnostics = Vec::new();

            // Find specific `new` expressions to report
            let mut cursor = QueryCursor::new();
            let constructor_idx = self
                .new_query
                .capture_index_for_name("constructor")
                .unwrap();
            let matches = cursor.matches(
                &self.new_query,
                file.tree.root_node(),
                source.as_slice(),
            );

            for m in matches {
                for cap in m.captures {
                    if cap.index == constructor_idx {
                        let name = cap.node.utf8_text(source).unwrap_or("?");
                        let line = cap.node.start_position().row + 1;
                        diagnostics.push(Diagnostic {
                            id: format!("EDR-{:03}", diagnostics.len() + 1),
                            severity: if edr < 0.2 {
                                Severity::Warning
                            } else {
                                Severity::Info
                            },
                            metric: "edr".into(),
                            file: file.path.display().to_string(),
                            line,
                            message: format!(
                                "Hardcoded dependency: `new {name}()` (EDR = {edr:.2})"
                            ),
                            suggestion: "Inject via constructor parameter for better testability"
                                .into(),
                        });
                    }
                }
            }

            diagnostics
        } else {
            Vec::new()
        }
    }
}

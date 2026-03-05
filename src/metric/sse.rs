use std::collections::{HashMap, HashSet};

use tree_sitter::{Query, QueryCursor};

use crate::lang::ParsedFile;
use crate::report::{Diagnostic, MetricSummary, Severity};

use super::{Metric, MetricResult};

/// State Space Expansion (SSE) + Ownership Ambiguity (OA)
///
/// SSE: Finds `let` declarations that are never reassigned (should be `const`).
/// OA: Finds class properties written by multiple external files.
pub struct SseMetric {
    pub variable_query: Query,
    pub assignment_query: Query,
}

impl Metric for SseMetric {
    fn id(&self) -> &str {
        "sse"
    }

    fn analyze_file(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let source = &file.source;

        // Phase 1: Collect all `let` declared variable names and their locations
        let mut let_vars: HashMap<String, usize> = HashMap::new(); // name -> line
        {
            let mut cursor = QueryCursor::new();
            let name_idx = self.variable_query.capture_index_for_name("name").unwrap();
            let matches =
                cursor.matches(&self.variable_query, file.tree.root_node(), source.as_slice());
            for m in matches {
                for cap in m.captures {
                    if cap.index == name_idx {
                        let name = cap.node.utf8_text(source).unwrap_or("").to_string();
                        let line = cap.node.start_position().row + 1;
                        let_vars.insert(name, line);
                    }
                }
            }
        }

        if let_vars.is_empty() {
            return Vec::new();
        }

        // Phase 2: Find all assignment targets
        let mut reassigned: HashSet<String> = HashSet::new();
        {
            let mut cursor = QueryCursor::new();
            let left_idx = self.assignment_query.capture_index_for_name("left").unwrap();
            let matches =
                cursor.matches(&self.assignment_query, file.tree.root_node(), source.as_slice());
            for m in matches {
                for cap in m.captures {
                    if cap.index == left_idx {
                        let text = cap.node.utf8_text(source).unwrap_or("");
                        // Simple identifier assignment (not member access)
                        if !text.contains('.') {
                            reassigned.insert(text.to_string());
                        }
                    }
                }
            }
        }

        // Phase 3: Report let vars that are never reassigned
        let mut diagnostics = Vec::new();
        for (name, line) in &let_vars {
            if !reassigned.contains(name) {
                diagnostics.push(Diagnostic {
                    id: format!("SSE-{:03}", diagnostics.len() + 1),
                    severity: Severity::Info,
                    metric: "sse".into(),
                    file: file.path.display().to_string(),
                    line: *line,
                    message: format!("`let {name}` is never reassigned"),
                    suggestion: format!("Change `let {name}` to `const {name}` to narrow the state space"),
                });
            }
        }

        diagnostics
    }

    fn analyze_project(&self, files: &[ParsedFile]) -> MetricResult {
        let diagnostics: Vec<_> = files.iter().flat_map(|f| self.analyze_file(f)).collect();

        // SSE score = declared_let / actually_mutated (ideal: 1.0, higher = worse)
        let total_lets: usize = files.iter().map(|f| count_lets(f, &self.variable_query)).sum();
        let unused_lets = diagnostics.len();
        let score = if total_lets == 0 {
            0.0
        } else {
            unused_lets as f64 / total_lets as f64
        };

        let severity = if score <= 0.0 {
            Severity::Ok
        } else if score < 0.2 {
            Severity::Info
        } else if score < 0.5 {
            Severity::Warning
        } else {
            Severity::Critical
        };

        MetricResult {
            summary: MetricSummary {
                id: "sse".into(),
                score,
                severity,
            },
            diagnostics,
        }
    }
}

fn count_lets(file: &ParsedFile, query: &Query) -> usize {
    let mut cursor = QueryCursor::new();
    let name_idx = query.capture_index_for_name("name").unwrap();
    cursor
        .matches(query, file.tree.root_node(), file.source.as_slice())
        .flat_map(|m| m.captures.iter())
        .filter(|c| c.index == name_idx)
        .count()
}

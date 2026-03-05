use petgraph::graph::NodeIndex;
use petgraph::visit::Bfs;
use tree_sitter::Query;

use crate::lang::ParsedFile;
use crate::report::{Diagnostic, MetricSummary, Severity};

use super::tce::build_import_graph;
use super::{Metric, MetricResult};

/// Transitive Closure Radius (TCR)
/// Measures how many modules are transitively affected by changing a given module.
/// TCR(M) = |reachable(M)| / total_modules
pub struct TcrMetric {
    pub import_query: Query,
}

impl Metric for TcrMetric {
    fn id(&self) -> &str {
        "tcr"
    }

    fn analyze_project(&self, files: &[ParsedFile]) -> MetricResult {
        let (graph, node_map, _index_to_path) = build_import_graph(files, &self.import_query);
        let total = node_map.len();

        if total == 0 {
            return MetricResult {
                summary: MetricSummary {
                    id: "tcr".into(),
                    score: 0.0,
                    severity: Severity::Ok,
                },
                diagnostics: Vec::new(),
            };
        }

        // For each node, BFS to count reachable nodes
        let mut diagnostics = Vec::new();
        let mut max_tcr: f64 = 0.0;

        let mut tcr_values: Vec<(String, f64)> = Vec::new();

        for (path, &node_idx) in &node_map {
            let reachable = bfs_reachable_count(&graph, node_idx);
            let tcr = reachable as f64 / total as f64;
            tcr_values.push((path.clone(), tcr));
            max_tcr = max_tcr.max(tcr);
        }

        // Sort by TCR descending, report high outliers
        tcr_values.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        for (path, tcr) in &tcr_values {
            if *tcr < 0.3 {
                break; // Only report significant ones
            }
            let severity = if *tcr >= 0.6 {
                Severity::Critical
            } else {
                Severity::Warning
            };

            // Extract short filename for readability
            diagnostics.push(Diagnostic {
                id: format!("TCR-{:03}", diagnostics.len() + 1),
                severity,
                metric: "tcr".into(),
                file: path.clone(),
                line: 0,
                message: format!(
                    "Change radius: {:.0}% of modules ({} / {}) are transitively affected",
                    tcr * 100.0,
                    (tcr * total as f64).round() as usize,
                    total
                ),
                suggestion:
                    "Consider reducing outgoing dependencies or extracting stable abstractions"
                        .into(),
            });
        }

        let severity = if max_tcr <= 0.1 {
            Severity::Ok
        } else if max_tcr < 0.3 {
            Severity::Info
        } else if max_tcr < 0.6 {
            Severity::Warning
        } else {
            Severity::Critical
        };

        MetricResult {
            summary: MetricSummary {
                id: "tcr".into(),
                score: max_tcr,
                severity,
            },
            diagnostics,
        }
    }
}

fn bfs_reachable_count(
    graph: &petgraph::graph::DiGraph<String, ()>,
    start: NodeIndex,
) -> usize {
    let mut bfs = Bfs::new(graph, start);
    let mut count = 0usize;
    while let Some(_) = bfs.next(graph) {
        count += 1;
    }
    // Subtract 1 because BFS includes the start node itself
    count.saturating_sub(1)
}

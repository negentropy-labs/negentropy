use std::collections::HashMap;
use std::path::Path;

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use tree_sitter::{Query, QueryCursor};

use crate::lang::ParsedFile;
use crate::report::{Diagnostic, MetricSummary, Severity};

use super::{Metric, MetricResult};

/// Topological Coupling Entropy (TCE)
/// Detects circular dependencies via SCC density.
/// TCE = size(largest_SCC) / total_modules
pub struct TceMetric {
    pub import_query: Query,
}

impl Metric for TceMetric {
    fn id(&self) -> &str {
        "tce"
    }

    fn analyze_project(&self, files: &[ParsedFile]) -> MetricResult {
        let (graph, node_map, index_to_path) = build_import_graph(files, &self.import_query);

        let sccs = tarjan_scc(&graph);
        let total = node_map.len();

        let mut diagnostics = Vec::new();
        let mut largest_scc_size = 0usize;

        for scc in &sccs {
            if scc.len() > 1 {
                largest_scc_size = largest_scc_size.max(scc.len());
                let cycle_files: Vec<_> = scc
                    .iter()
                    .filter_map(|idx| index_to_path.get(idx))
                    .collect();
                let file_list = cycle_files
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(" <-> ");

                for path in &cycle_files {
                    diagnostics.push(Diagnostic {
                        id: format!("TCE-{:03}", diagnostics.len() + 1),
                        severity: Severity::Critical,
                        metric: "tce".into(),
                        file: path.to_string(),
                        line: 0,
                        message: format!(
                            "Circular dependency cycle ({} modules): {}",
                            scc.len(),
                            file_list
                        ),
                        suggestion: "Extract shared interface into a third module, or use event-based decoupling".into(),
                    });
                }
            }
        }

        let score = if total == 0 {
            0.0
        } else {
            largest_scc_size as f64 / total as f64
        };

        let severity = if score <= 0.0 {
            Severity::Ok
        } else if score < 0.1 {
            Severity::Info
        } else if score < 0.3 {
            Severity::Warning
        } else {
            Severity::Critical
        };

        MetricResult {
            summary: MetricSummary {
                id: "tce".into(),
                score,
                severity,
            },
            diagnostics,
        }
    }
}

/// Build a directed graph from import relationships.
/// Returns (graph, path->node_index map, node_index->path map).
pub fn build_import_graph(
    files: &[ParsedFile],
    import_query: &Query,
) -> (
    DiGraph<String, ()>,
    HashMap<String, NodeIndex>,
    HashMap<NodeIndex, String>,
) {
    let mut graph = DiGraph::new();
    let mut node_map: HashMap<String, NodeIndex> = HashMap::new();
    let mut index_to_path: HashMap<NodeIndex, String> = HashMap::new();

    // Register all files as nodes (canonicalize paths for consistent matching)
    for file in files {
        let canonical = normalize_path(&file.path);
        let display = file.path.display().to_string();
        let idx = graph.add_node(display.clone());
        node_map.insert(canonical, idx);
        index_to_path.insert(idx, display);
    }

    let source_idx = import_query.capture_index_for_name("source").unwrap();

    // Add edges from imports
    for file in files {
        let from_key = normalize_path(&file.path);
        let from_idx = node_map[&from_key];

        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(
            import_query,
            file.tree.root_node(),
            file.source.as_slice(),
        );

        for m in matches {
            for cap in m.captures {
                if cap.index != source_idx {
                    continue;
                }
                let raw = cap.node.utf8_text(&file.source).unwrap_or("");
                let import_path = raw.trim_matches(|c| c == '\'' || c == '"');

                // Only resolve relative imports
                if !import_path.starts_with('.') {
                    continue;
                }

                if let Some(resolved) = resolve_import(&file.path, import_path) {
                    // Try exact match or with extensions
                    let target = find_target(&resolved, &node_map);
                    if let Some(to_idx) = target {
                        graph.add_edge(from_idx, to_idx, ());
                    }
                }
            }
        }
    }

    (graph, node_map, index_to_path)
}

fn resolve_import(from_file: &Path, import_path: &str) -> Option<String> {
    let dir = from_file.parent()?;
    let resolved = dir.join(import_path);
    Some(normalize_path(&resolved))
}

/// Normalize a path by resolving `.` and `..` components without filesystem access.
fn normalize_path(path: &Path) -> String {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    let normalized: std::path::PathBuf = components.iter().collect();
    normalized.display().to_string()
}

fn find_target(base: &str, node_map: &HashMap<String, NodeIndex>) -> Option<NodeIndex> {
    // Try exact match first
    if let Some(idx) = node_map.get(base) {
        return Some(*idx);
    }
    // Try with common extensions
    for ext in &[".ts", ".tsx", "/index.ts", "/index.tsx"] {
        let with_ext = format!("{base}{ext}");
        if let Some(idx) = node_map.get(&with_ext) {
            return Some(*idx);
        }
    }
    None
}

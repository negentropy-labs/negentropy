use tree_sitter::{Query, QueryCursor};

use crate::lang::ParsedFile;
use crate::report::{Diagnostic, Severity};

use super::Metric;

/// Physical-Logical Mapping Entropy (PLME)
/// Penalizes deep relative imports (../../..) that couple code to physical layout.
pub struct PlmeMetric {
    pub import_query: Query,
}

impl Metric for PlmeMetric {
    fn id(&self) -> &str {
        "plme"
    }

    fn analyze_file(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut cursor = QueryCursor::new();
        let source = &file.source;
        let matches = cursor.matches(&self.import_query, file.tree.root_node(), source.as_slice());

        let source_idx = self.import_query.capture_index_for_name("source").unwrap();
        let mut diagnostics = Vec::new();

        for m in matches {
            for cap in m.captures {
                if cap.index != source_idx {
                    continue;
                }
                let text = cap.node.utf8_text(source).unwrap_or("");
                // Strip quotes
                let path = text.trim_matches(|c| c == '\'' || c == '"');
                let depth = path.matches("../").count();
                if depth >= 2 {
                    let line = cap.node.start_position().row + 1;
                    let severity = if depth >= 4 {
                        Severity::Critical
                    } else if depth >= 3 {
                        Severity::Warning
                    } else {
                        Severity::Info
                    };
                    diagnostics.push(Diagnostic {
                        id: format!("PLME-{:03}", diagnostics.len() + 1),
                        severity,
                        metric: "plme".into(),
                        file: file.path.display().to_string(),
                        line,
                        message: format!("Relative import depth {depth}: {path}"),
                        suggestion: "Use path alias (e.g. @services/...) to decouple from physical layout".into(),
                    });
                }
            }
        }

        diagnostics
    }
}

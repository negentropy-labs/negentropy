pub mod plme;
pub mod sse;
pub mod tce;
pub mod edr;
pub mod iie;
pub mod ead;
pub mod tcr;

use crate::lang::{LanguageSupport, ParsedFile, QueryKind};
use crate::report::{Diagnostic, MetricSummary};

/// Result of a single metric's analysis.
pub struct MetricResult {
    pub summary: MetricSummary,
    pub diagnostics: Vec<Diagnostic>,
}

/// Every metric is self-contained: it owns its pre-compiled queries,
/// extracts facts, and computes results independently.
pub trait Metric {
    fn id(&self) -> &str;

    /// Single-file analysis. Default: no diagnostics.
    fn analyze_file(&self, _file: &ParsedFile) -> Vec<Diagnostic> {
        Vec::new()
    }

    /// Project-level analysis. Override for metrics that need the full graph (TCE, TCR).
    fn analyze_project(&self, files: &[ParsedFile]) -> MetricResult {
        let diagnostics: Vec<_> = files
            .iter()
            .flat_map(|f| self.analyze_file(f))
            .collect();
        let score = self.score_from_diagnostics(&diagnostics);
        MetricResult {
            summary: MetricSummary {
                id: self.id().to_string(),
                score,
                severity: severity_from_score(score),
            },
            diagnostics,
        }
    }

    /// Compute a 0.0..1.0 score from diagnostics. Default: ratio of files with issues.
    fn score_from_diagnostics(&self, diagnostics: &[Diagnostic]) -> f64 {
        if diagnostics.is_empty() {
            return 0.0;
        }
        // Higher = worse. Count unique files with issues.
        let unique_files: std::collections::HashSet<_> =
            diagnostics.iter().map(|d| &d.file).collect();
        unique_files.len() as f64
    }
}

fn severity_from_score(score: f64) -> crate::report::Severity {
    use crate::report::Severity;
    if score <= 0.0 {
        Severity::Ok
    } else if score < 0.3 {
        Severity::Info
    } else if score < 0.6 {
        Severity::Warning
    } else {
        Severity::Critical
    }
}

/// Build all metrics with pre-compiled queries from the language.
pub fn build_metrics(lang: &dyn LanguageSupport) -> Vec<Box<dyn Metric>> {
    let ts_lang = lang.language();
    let compile = |kind: QueryKind| -> tree_sitter::Query {
        tree_sitter::Query::new(&ts_lang, lang.query_source(kind))
            .unwrap_or_else(|e| panic!("bad query for {kind:?}: {e}"))
    };

    vec![
        Box::new(plme::PlmeMetric {
            import_query: compile(QueryKind::Imports),
        }),
        Box::new(sse::SseMetric {
            variable_query: compile(QueryKind::VariableDeclarations),
            assignment_query: compile(QueryKind::Assignments),
        }),
        Box::new(tce::TceMetric {
            import_query: compile(QueryKind::Imports),
        }),
        Box::new(edr::EdrMetric {
            constructor_query: compile(QueryKind::ConstructorParams),
            new_query: compile(QueryKind::NewExpressions),
        }),
        Box::new(iie::IieMetric {
            export_query: compile(QueryKind::Exports),
        }),
        Box::new(ead::EadMetric {
            member_query: compile(QueryKind::MemberAccesses),
            class_query: compile(QueryKind::ClassDeclarations),
        }),
        Box::new(tcr::TcrMetric {
            import_query: compile(QueryKind::Imports),
        }),
    ]
}

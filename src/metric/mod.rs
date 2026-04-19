pub mod ead;
pub mod edr;
pub mod iie;
pub mod plme;
pub mod sse;
pub mod tce;
pub mod tcr;

use crate::context::ProjectContext;
use crate::report::{Diagnostic, DimensionSummary, Hotspot};

/// Result of a single metric's analysis.
pub struct MetricResult {
    pub dimension: DimensionSummary,
    pub hotspots: Vec<Hotspot>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Metrics consume the shared project context instead of re-parsing files.
pub trait Metric {
    fn id(&self) -> &str;

    fn analyze_project(&self, ctx: &ProjectContext, top_n: usize) -> MetricResult;
}

pub fn build_metrics() -> Vec<Box<dyn Metric>> {
    vec![
        Box::new(iie::IieMetric),
        Box::new(ead::EadMetric),
        Box::new(tcr::TcrMetric),
        Box::new(tce::TceMetric),
        Box::new(edr::EdrMetric),
        Box::new(plme::PlmeMetric),
        Box::new(sse::SseMetric),
    ]
}

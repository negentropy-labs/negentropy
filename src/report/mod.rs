use serde::Serialize;

use crate::model::{ComputedMetrics, Dimension, Hotspot, RiskLevel};

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub files_scanned: usize,
    pub modules: usize,
    pub overall_risk: RiskLevel,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisReport {
    pub tool_version: String,
    pub target_path: String,
    pub effective_extensions: Vec<String>,
    pub summary: Summary,
    pub dimensions: Vec<Dimension>,
    pub hotspots: Vec<Hotspot>,
}

impl AnalysisReport {
    pub fn new(
        target_path: String,
        effective_extensions: Vec<String>,
        files_scanned: usize,
        modules: usize,
        metrics: ComputedMetrics,
    ) -> Self {
        Self {
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            target_path,
            effective_extensions,
            summary: Summary {
                files_scanned,
                modules,
                overall_risk: metrics.overall_risk,
            },
            dimensions: metrics.dimensions,
            hotspots: metrics.hotspots,
        }
    }
}

pub fn render_table(report: &AnalysisReport) -> String {
    let mut out = String::new();
    out.push_str("Negentropy V2 Report\n");
    out.push_str(&format!("Target: {}\n", report.target_path));
    out.push_str(&format!(
        "Extensions: {}\n",
        report.effective_extensions.join(",")
    ));
    out.push_str(&format!(
        "Files: {}  Modules: {}  Overall Risk: {:?}\n\n",
        report.summary.files_scanned, report.summary.modules, report.summary.overall_risk
    ));

    out.push_str("Dimensions\n");
    out.push_str("- id | metric | risk | raw\n");
    for dim in &report.dimensions {
        out.push_str(&format!(
            "- {} | {} | {:?} | {}\n",
            dim.id, dim.metric, dim.risk, dim.raw
        ));
    }

    out.push_str("\nHotspots\n");
    for hs in &report.hotspots {
        out.push_str(&format!(
            "- [{}] {} = {} @ {} ({})\n",
            hs.dimension_id, hs.entity, hs.metric_value, hs.location, hs.reason
        ));
    }

    out
}

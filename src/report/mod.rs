mod delta;

use serde::{Deserialize, Serialize};

use crate::model::{ComputedMetrics, Dimension, Hotspot, RiskLevel};

pub use delta::ReportDelta;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub files_scanned: usize,
    pub modules: usize,
    pub overall_risk: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub tool_version: String,
    pub target_path: String,
    pub effective_extensions: Vec<String>,
    pub summary: Summary,
    pub dimensions: Vec<Dimension>,
    pub hotspots: Vec<Hotspot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<ReportDelta>,
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
            delta: None,
        }
    }

    pub fn with_delta(mut self, baseline_path: String, baseline: &AnalysisReport) -> Self {
        self.delta = Some(ReportDelta::between(baseline_path, baseline, &self));
        self
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

    if let Some(delta) = &report.delta {
        out.push_str("\nDelta\n");
        out.push_str(&format!("Baseline: {}\n", delta.baseline_path));
        out.push_str("- id | raw delta | risk delta\n");
        for dimension in &delta.dimensions {
            let raw_delta = dimension
                .raw_delta
                .as_ref()
                .map_or_else(|| "n/a".to_string(), ToString::to_string);
            let risk_delta = dimension
                .risk_delta
                .map_or_else(|| "n/a".to_string(), |value| format!("{value:+}"));
            out.push_str(&format!(
                "- {} | {} | {}\n",
                dimension.id, raw_delta, risk_delta
            ));
        }

        out.push_str(&format!("\nNew Hotspots: {}\n", delta.new_hotspots.len()));
        for hs in &delta.new_hotspots {
            out.push_str(&format!(
                "- [{}] {} = {} @ {} ({})\n",
                hs.dimension_id, hs.entity, hs.metric_value, hs.location, hs.reason
            ));
        }

        out.push_str(&format!(
            "\nResolved Hotspots: {}\n",
            delta.resolved_hotspots.len()
        ));
        for hs in &delta.resolved_hotspots {
            out.push_str(&format!(
                "- [{}] {} = {} @ {} ({})\n",
                hs.dimension_id, hs.entity, hs.metric_value, hs.location, hs.reason
            ));
        }
    }

    out
}

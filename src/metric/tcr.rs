use serde_json::json;

use crate::context::ProjectContext;
use crate::report::*;

use super::{Metric, MetricResult};

/// Transitive Closure Radius (TCR)
pub struct TcrMetric;

impl Metric for TcrMetric {
    fn id(&self) -> &str {
        "change_blast_radius"
    }

    fn analyze_project(&self, ctx: &ProjectContext, top_n: usize) -> MetricResult {
        let max_tcr = ctx
            .graph
            .tcr_by_module
            .first()
            .map(|(_, value)| *value)
            .unwrap_or(0.0);
        let risk = risk_ascending(max_tcr, 0.10, 0.30);

        let hotspots = ctx
            .graph
            .tcr_by_module
            .iter()
            .take(top_n)
            .map(|(file, score)| Hotspot {
                dimension_id: self.id().into(),
                entity: file.clone(),
                metric_value: round3(*score),
                location: "module".into(),
                reason: "Large reverse transitive impact radius".into(),
            })
            .collect::<Vec<_>>();

        let diagnostics = ctx
            .graph
            .tcr_by_module
            .iter()
            .filter(|(_, score)| *score >= 0.3)
            .enumerate()
            .map(|(idx, (file, score))| Diagnostic {
                id: format!("TCR-{:03}", idx + 1),
                risk: if *score >= 0.6 {
                    RiskLevel::High
                } else {
                    RiskLevel::Medium
                },
                metric: "TCR".into(),
                file: file.clone(),
                line: 0,
                message: format!("Change radius: {:.0}% of modules affected", score * 100.0),
                suggestion: "Reduce outgoing dependencies or extract stable abstractions".into(),
            })
            .collect::<Vec<_>>();

        MetricResult {
            dimension: DimensionSummary {
                id: self.id().into(),
                metric: "TCR".into(),
                raw: json!(round3(max_tcr)),
                risk,
            },
            hotspots,
            diagnostics,
        }
    }
}

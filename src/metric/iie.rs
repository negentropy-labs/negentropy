use serde_json::json;

use crate::context::ProjectContext;
use crate::report::*;

use super::{Metric, MetricResult};

/// Interface-Implementation Entropy (IIE)
pub struct IieMetric;

impl Metric for IieMetric {
    fn id(&self) -> &str {
        "module_abstraction"
    }

    fn analyze_project(&self, ctx: &ProjectContext, top_n: usize) -> MetricResult {
        let mut diagnostics = Vec::new();
        let mut hotspots = Vec::new();
        let mut values = Vec::new();

        for fact in &ctx.facts {
            for export in &fact.exports {
                values.push(export.iie);

                if export.iie <= 1.0 {
                    continue;
                }

                hotspots.push(Hotspot {
                    dimension_id: self.id().into(),
                    entity: export.snippet.clone(),
                    metric_value: round3(export.iie),
                    location: format!("{}:{}", fact.file, export.line),
                    reason: format!(
                        "interface ({}) > implementation ({})",
                        export.signature_nodes, export.body_nodes
                    ),
                });

                diagnostics.push(Diagnostic {
                    id: format!("IIE-{:03}", diagnostics.len() + 1),
                    risk: if export.iie > 2.0 {
                        RiskLevel::High
                    } else {
                        RiskLevel::Medium
                    },
                    metric: "IIE".into(),
                    file: fact.file.clone(),
                    line: export.line,
                    message: format!(
                        "Shallow module (IIE = {:.2}): interface ({}) > implementation ({})",
                        export.iie, export.signature_nodes, export.body_nodes
                    ),
                    suggestion:
                        "This export may be a trivial wrapper. Consider inlining or deepening its implementation"
                            .into(),
                });
            }
        }

        hotspots.sort_by(|a, b| b.metric_value.total_cmp(&a.metric_value));
        hotspots.truncate(top_n);

        let median_iie = median(values);
        let risk = risk_ascending(median_iie, 0.60, 1.00);

        MetricResult {
            dimension: DimensionSummary {
                id: self.id().into(),
                metric: "IIE".into(),
                raw: json!(round3(median_iie)),
                risk,
            },
            hotspots,
            diagnostics,
        }
    }
}

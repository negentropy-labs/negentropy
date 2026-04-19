use serde_json::json;

use crate::context::ProjectContext;
use crate::report::*;

use super::{Metric, MetricResult};

/// External Attribute Dependency (EAD)
pub struct EadMetric;

impl Metric for EadMetric {
    fn id(&self) -> &str {
        "logic_cohesion"
    }

    fn analyze_project(&self, ctx: &ProjectContext, top_n: usize) -> MetricResult {
        let mut diagnostics = Vec::new();
        let mut hotspots = Vec::new();
        let mut values = Vec::new();

        for fact in &ctx.facts {
            for entity in &fact.ead_entities {
                values.push(entity.ead as f64);

                if entity.ead <= 2 {
                    continue;
                }

                hotspots.push(Hotspot {
                    dimension_id: self.id().into(),
                    entity: entity.entity.clone(),
                    metric_value: entity.ead as f64,
                    location: format!("{}:{}", fact.file, entity.line),
                    reason: format!(
                        "{} external vs {} self accesses",
                        entity.external_accesses, entity.self_accesses
                    ),
                });

                diagnostics.push(Diagnostic {
                    id: format!("EAD-{:03}", diagnostics.len() + 1),
                    risk: if entity.ead > 5 {
                        RiskLevel::High
                    } else {
                        RiskLevel::Medium
                    },
                    metric: "EAD".into(),
                    file: fact.file.clone(),
                    line: entity.line,
                    message: format!(
                        "Feature Envy in `{}`: {} external vs {} self property accesses (EAD = {})",
                        entity.entity, entity.external_accesses, entity.self_accesses, entity.ead
                    ),
                    suggestion: "Consider moving this logic closer to the data it accesses".into(),
                });
            }
        }

        hotspots.sort_by(|a, b| b.metric_value.total_cmp(&a.metric_value));
        hotspots.truncate(top_n);

        let median_ead = median(values);
        let risk = risk_ascending(median_ead, 1.0, 3.0);

        MetricResult {
            dimension: DimensionSummary {
                id: self.id().into(),
                metric: "EAD".into(),
                raw: json!(round3(median_ead)),
                risk,
            },
            hotspots,
            diagnostics,
        }
    }
}

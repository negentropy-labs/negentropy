use std::collections::HashSet;

use serde_json::json;

use crate::context::ProjectContext;
use crate::report::*;

use super::{Metric, MetricResult};

/// Physical-Logical Mapping Entropy (PLME)
pub struct PlmeMetric;

impl Metric for PlmeMetric {
    fn id(&self) -> &str {
        "intent_redundancy"
    }

    fn analyze_project(&self, ctx: &ProjectContext, top_n: usize) -> MetricResult {
        let mut diagnostics = Vec::new();
        let mut hotspots = Vec::new();
        let mut depth_sum = 0usize;
        let mut unique_targets = HashSet::new();

        for fact in &ctx.facts {
            for import in &fact.imports {
                depth_sum += import.distance;
                unique_targets.insert(
                    import
                        .resolved_target
                        .clone()
                        .unwrap_or_else(|| import.raw_target.clone()),
                );

                if import.distance < 2 {
                    continue;
                }

                hotspots.push(Hotspot {
                    dimension_id: self.id().into(),
                    entity: format!("{} -> {}", fact.file, import.raw_target),
                    metric_value: import.distance as f64,
                    location: format!("{}:{}", fact.file, import.line),
                    reason: "Deep relative import path".into(),
                });

                diagnostics.push(Diagnostic {
                    id: format!("PLME-{:03}", diagnostics.len() + 1),
                    risk: if import.distance >= 4 {
                        RiskLevel::High
                    } else if import.distance >= 3 {
                        RiskLevel::Medium
                    } else {
                        RiskLevel::Low
                    },
                    metric: "PLME".into(),
                    file: fact.file.clone(),
                    line: import.line,
                    message: format!(
                        "Relative import depth {}: {}",
                        import.distance, import.raw_target
                    ),
                    suggestion:
                        "Use path alias (e.g. @services/...) to decouple from physical layout"
                            .into(),
                });
            }
        }

        hotspots.sort_by(|a, b| b.metric_value.total_cmp(&a.metric_value));
        hotspots.truncate(top_n);

        let plme = depth_sum as f64 / unique_targets.len().max(1) as f64;
        let risk = risk_ascending(plme, 0.40, 1.20);

        MetricResult {
            dimension: DimensionSummary {
                id: self.id().into(),
                metric: "PLME".into(),
                raw: json!(round3(plme)),
                risk,
            },
            hotspots,
            diagnostics,
        }
    }
}

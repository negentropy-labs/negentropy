use serde_json::json;

use crate::context::ProjectContext;
use crate::report::*;

use super::{Metric, MetricResult};

/// Explicit Dependency Rate (EDR)
pub struct EdrMetric;

impl Metric for EdrMetric {
    fn id(&self) -> &str {
        "testability_pluggability"
    }

    fn analyze_project(&self, ctx: &ProjectContext, top_n: usize) -> MetricResult {
        let mut total_injected = 0usize;
        let mut total_hardcoded = 0usize;
        let mut diagnostics = Vec::new();
        let mut hotspots = Vec::new();

        for fact in &ctx.facts {
            let injected = fact.constructor_params;
            let hardcoded = fact.new_expressions.len();

            total_injected += injected;
            total_hardcoded += hardcoded;

            let file_total = injected + hardcoded;
            if file_total == 0 {
                continue;
            }

            let file_edr = injected as f64 / file_total as f64;
            if hardcoded > 0 && file_edr < 0.5 {
                let pressure = hardcoded as f64 / file_total as f64;
                hotspots.push(Hotspot {
                    dimension_id: self.id().into(),
                    entity: fact.file.clone(),
                    metric_value: round3(pressure),
                    location: "file".into(),
                    reason: "Hardcoded dependency pressure".into(),
                });

                for new_expr in &fact.new_expressions {
                    diagnostics.push(Diagnostic {
                        id: format!("EDR-{:03}", diagnostics.len() + 1),
                        risk: if file_edr < 0.2 {
                            RiskLevel::Medium
                        } else {
                            RiskLevel::Low
                        },
                        metric: "EDR".into(),
                        file: fact.file.clone(),
                        line: new_expr.line,
                        message: format!(
                            "Hardcoded dependency: `new {}()` (EDR = {:.2})",
                            new_expr.constructor, file_edr
                        ),
                        suggestion: "Inject via constructor parameter for better testability"
                            .into(),
                    });
                }
            }
        }

        let total = total_injected + total_hardcoded;
        let edr = if total == 0 {
            1.0
        } else {
            total_injected as f64 / total as f64
        };
        let risk = risk_descending(edr, 0.70, 0.40);

        hotspots.sort_by(|a, b| b.metric_value.total_cmp(&a.metric_value));
        hotspots.truncate(top_n);

        MetricResult {
            dimension: DimensionSummary {
                id: self.id().into(),
                metric: "EDR".into(),
                raw: json!(round3(edr)),
                risk,
            },
            hotspots,
            diagnostics,
        }
    }
}

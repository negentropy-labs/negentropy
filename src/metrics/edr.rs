use serde_json::json;

use crate::context::ProjectContext;
use crate::model::{Dimension, Hotspot};

use super::{metric_output, positive_hotspots, risk_descending, round3};

pub(super) fn compute(context: &ProjectContext, top_n: usize) -> super::MetricOutput {
    let mut total_injected = 0usize;
    let mut total_hardcoded = 0usize;
    let mut hotspots = Vec::new();

    for file in &context.facts {
        for func in &file.functions {
            total_injected += func.injected_interactions;
            total_hardcoded += func.hardcoded_interactions;

            let total = func.injected_interactions + func.hardcoded_interactions;
            let pressure = func.hardcoded_interactions as f64 / total.max(1) as f64;
            hotspots.push(Hotspot {
                dimension_id: "testability_pluggability".to_string(),
                entity: format!("{}::{}", file.module_id, func.name),
                metric_value: round3(pressure),
                location: format!("{}:{}", file.module_id, func.line),
                reason: "Hardcoded dependency pressure".to_string(),
            });
        }
    }

    let total_interactions = total_injected + total_hardcoded;
    let edr = if total_interactions == 0 {
        1.0
    } else {
        total_injected as f64 / total_interactions as f64
    };
    let risk = risk_descending(edr, 0.70, 0.40);

    metric_output(
        Dimension {
            id: "testability_pluggability".to_string(),
            metric: "EDR".to_string(),
            raw: json!(round3(edr)),
            risk,
        },
        positive_hotspots(hotspots, top_n),
    )
}

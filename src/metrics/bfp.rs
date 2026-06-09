use std::collections::HashSet;

use serde_json::json;

use crate::context::ProjectContext;
use crate::model::{Dimension, Hotspot};

use super::{metric_output, percentile, positive_hotspots, risk_ascending, round3};

pub(super) fn compute(context: &ProjectContext, top_n: usize) -> super::MetricOutput {
    let mut scores = Vec::new();
    let mut hotspots = Vec::new();

    for file in &context.facts {
        if file.module.is_generated_like {
            continue;
        }

        for function in &file.functions {
            let flag_params = function
                .params
                .iter()
                .filter(|param| param.boolean_like)
                .map(|param| param.name.as_str())
                .collect::<HashSet<_>>();
            let flag_controlled_branches = function
                .branches
                .iter()
                .filter(|branch| {
                    branch
                        .referenced_params
                        .iter()
                        .any(|param| flag_params.contains(param.as_str()))
                })
                .count();

            let pressure = flag_params.len() as f64
                + flag_controlled_branches as f64
                + flag_params.len().saturating_sub(1) as f64 * 2.0;

            scores.push(pressure);
            hotspots.push(Hotspot {
                dimension_id: "behavior_mode_pressure".to_string(),
                entity: format!("{}::{}", file.module_id, function.name),
                metric_value: round3(pressure),
                location: format!("{}:{}", file.module_id, function.line),
                reason: format!(
                    "{} boolean-like params, {} flag-controlled branches",
                    flag_params.len(),
                    flag_controlled_branches
                ),
            });
        }

        for call in &file.calls {
            let pressure = call.boolean_literal_args.len() as f64 * 0.5;
            scores.push(pressure);
            hotspots.push(Hotspot {
                dimension_id: "behavior_mode_pressure".to_string(),
                entity: format!("{} -> {}", file.module_id, call.callee),
                metric_value: round3(pressure),
                location: format!("{}:{}", file.module_id, call.line),
                reason: format!(
                    "{} boolean literal args at call site",
                    call.boolean_literal_args.len()
                ),
            });
        }
    }

    let bfp = percentile(scores, 0.90);
    let risk = risk_ascending(bfp, 1.0, 3.0);

    metric_output(
        Dimension {
            id: "behavior_mode_pressure".to_string(),
            metric: "BFP".to_string(),
            raw: json!(round3(bfp)),
            risk,
        },
        positive_hotspots(hotspots, top_n),
    )
}

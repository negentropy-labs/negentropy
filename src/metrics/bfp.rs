use std::collections::HashMap;

use serde_json::json;

use crate::context::ProjectContext;
use crate::facts::BooleanEvidence;
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
                .filter_map(|param| {
                    param
                        .boolean_evidence
                        .map(|evidence| (param.name.as_str(), evidence))
                })
                .collect::<HashMap<_, _>>();
            let flag_controlled_branch_pressure = function
                .branches
                .iter()
                .filter_map(|branch| {
                    branch
                        .referenced_params
                        .iter()
                        .filter_map(|param| flag_params.get(param.as_str()))
                        .map(|evidence| evidence_weight(*evidence))
                        .max_by(f64::total_cmp)
                })
                .sum::<f64>();
            let param_pressure = flag_params
                .values()
                .map(|evidence| evidence_weight(*evidence))
                .sum::<f64>();
            let multi_flag_penalty = flag_params.len().saturating_sub(1) as f64;

            let pressure = param_pressure + flag_controlled_branch_pressure + multi_flag_penalty;
            let evidence_counts = EvidenceCounts::from(flag_params.values().copied());

            scores.push(pressure);
            hotspots.push(Hotspot {
                dimension_id: "behavior_mode_pressure".to_string(),
                entity: format!("{}::{}", file.module_id, function.name),
                metric_value: round3(pressure),
                location: format!("{}:{}", file.module_id, function.line),
                reason: format!(
                    "{} boolean-mode params ({}) with {} weighted flag branches",
                    flag_params.len(),
                    evidence_counts.describe(),
                    round3(flag_controlled_branch_pressure)
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

fn evidence_weight(evidence: BooleanEvidence) -> f64 {
    match evidence {
        BooleanEvidence::ExplicitType => 1.0,
        BooleanEvidence::LiteralDefault => 0.75,
        BooleanEvidence::NameHeuristic => 0.5,
    }
}

#[derive(Default)]
struct EvidenceCounts {
    explicit_type: usize,
    literal_default: usize,
    name_heuristic: usize,
}

impl EvidenceCounts {
    fn from(evidence: impl IntoIterator<Item = BooleanEvidence>) -> Self {
        let mut counts = Self::default();
        for evidence in evidence {
            match evidence {
                BooleanEvidence::ExplicitType => counts.explicit_type += 1,
                BooleanEvidence::LiteralDefault => counts.literal_default += 1,
                BooleanEvidence::NameHeuristic => counts.name_heuristic += 1,
            }
        }
        counts
    }

    fn describe(&self) -> String {
        format!(
            "{} explicit type, {} literal default, {} name heuristic",
            self.explicit_type, self.literal_default, self.name_heuristic
        )
    }
}

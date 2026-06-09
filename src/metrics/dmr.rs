use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::context::ProjectContext;
use crate::model::{Dimension, Hotspot};

use super::{metric_output, positive_hotspots, risk_ascending, round3};

pub(super) fn compute(context: &ProjectContext, top_n: usize) -> super::MetricOutput {
    if context.facts.len() <= 1 {
        return metric_output(
            Dimension {
                id: "module_reachability".to_string(),
                metric: "DMR".to_string(),
                raw: json!(0.0),
                risk: risk_ascending(0.0, 0.20, 0.50),
            },
            Vec::new(),
        );
    }

    let module_ids = context
        .facts
        .iter()
        .map(|fact| fact.module_id.as_str())
        .collect::<HashSet<_>>();
    let mut incoming: HashMap<&str, HashSet<&str>> = HashMap::new();

    for file in &context.facts {
        for import in &file.imports {
            if let Some(target) = import.resolved_target.as_deref()
                && module_ids.contains(target)
            {
                incoming
                    .entry(target)
                    .or_default()
                    .insert(file.module_id.as_str());
            }
        }
    }

    let candidates = context
        .facts
        .iter()
        .filter(|file| {
            !file.module.is_entry_like && !file.module.is_test && !file.module.is_generated_like
        })
        .collect::<Vec<_>>();

    let mut hotspots = Vec::new();
    for file in &candidates {
        if incoming
            .get(file.module_id.as_str())
            .is_none_or(HashSet::is_empty)
        {
            hotspots.push(Hotspot {
                dimension_id: "module_reachability".to_string(),
                entity: file.module_id.clone(),
                metric_value: 1.0,
                location: file.module_id.clone(),
                reason: "Module is not imported and is not an obvious entry/test/generated file"
                    .to_string(),
            });
        }
    }

    let dmr = hotspots.len() as f64 / candidates.len().max(1) as f64;
    let risk = risk_ascending(dmr, 0.20, 0.50);

    metric_output(
        Dimension {
            id: "module_reachability".to_string(),
            metric: "DMR".to_string(),
            raw: json!(round3(dmr)),
            risk,
        },
        positive_hotspots(hotspots, top_n),
    )
}

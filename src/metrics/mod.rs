use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::facts::FileFacts;
use crate::graph::GraphAnalysis;
use crate::model::{ComputedMetrics, Dimension, Hotspot, RiskLevel};

pub fn compute_metrics(
    facts: &[FileFacts],
    graph: &GraphAnalysis,
    top_n: usize,
) -> ComputedMetrics {
    let mut dimensions = Vec::new();
    let mut hotspots = Vec::new();

    let mut overall_risk = RiskLevel::Low;

    // 1) module_abstraction / IIE
    let mut iie_file_scores = facts
        .iter()
        .map(|f| {
            (
                f.module_id.clone(),
                f.export_complexity / f.implementation_complexity.max(1.0),
            )
        })
        .collect::<Vec<_>>();
    iie_file_scores.sort_by(|a, b| a.1.total_cmp(&b.1));
    let iie = median(iie_file_scores.iter().map(|(_, v)| *v).collect());
    let iie_risk = risk_ascending(iie, 0.35, 0.80);
    overall_risk = RiskLevel::max(overall_risk, iie_risk);
    dimensions.push(Dimension {
        id: "module_abstraction".to_string(),
        metric: "IIE".to_string(),
        raw: json!(round3(iie)),
        risk: iie_risk,
    });
    iie_file_scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (entity, value) in iie_file_scores.into_iter().take(top_n) {
        hotspots.push(Hotspot {
            dimension_id: "module_abstraction".to_string(),
            entity,
            metric_value: round3(value),
            location: "file".to_string(),
            reason: "High interface-to-implementation ratio".to_string(),
        });
    }

    // 2) logic_cohesion / EAD
    let mut function_ead = Vec::new();
    for file in facts {
        for func in &file.functions {
            function_ead.push((
                format!("{}::{}", file.module_id, func.name),
                func.ead,
                format!("{}:{}", file.module_id, func.line),
            ));
        }
    }
    let ead = percentile(
        function_ead.iter().map(|(_, v, _)| *v).collect::<Vec<_>>(),
        0.90,
    );
    let ead_risk = risk_ascending(ead, 1.0, 3.0);
    overall_risk = RiskLevel::max(overall_risk, ead_risk);
    dimensions.push(Dimension {
        id: "logic_cohesion".to_string(),
        metric: "EAD".to_string(),
        raw: json!(round3(ead)),
        risk: ead_risk,
    });
    function_ead.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (entity, value, location) in function_ead.into_iter().take(top_n) {
        hotspots.push(Hotspot {
            dimension_id: "logic_cohesion".to_string(),
            entity,
            metric_value: round3(value),
            location,
            reason: "Reads external attributes more than self attributes".to_string(),
        });
    }

    // 3) change_blast_radius / TCR
    let tcr = graph.tcr_by_module.first().map(|(_, v)| *v).unwrap_or(0.0);
    let tcr_risk = risk_ascending(tcr, 0.10, 0.30);
    overall_risk = RiskLevel::max(overall_risk, tcr_risk);
    dimensions.push(Dimension {
        id: "change_blast_radius".to_string(),
        metric: "TCR".to_string(),
        raw: json!(round3(tcr)),
        risk: tcr_risk,
    });
    for (entity, value) in graph.tcr_by_module.iter().take(top_n) {
        hotspots.push(Hotspot {
            dimension_id: "change_blast_radius".to_string(),
            entity: entity.clone(),
            metric_value: round3(*value),
            location: "module".to_string(),
            reason: "Large reverse transitive impact radius".to_string(),
        });
    }

    // 4) architecture_decoupling / TCE
    let tce = graph.tce;
    let tce_risk = risk_ascending(tce, 0.10, 0.30);
    overall_risk = RiskLevel::max(overall_risk, tce_risk);
    dimensions.push(Dimension {
        id: "architecture_decoupling".to_string(),
        metric: "TCE".to_string(),
        raw: json!(round3(tce)),
        risk: tce_risk,
    });

    // 5) testability_pluggability / EDR
    let mut total_injected = 0usize;
    let mut total_hardcoded = 0usize;
    let mut edr_hotspots = Vec::new();
    for file in facts {
        for func in &file.functions {
            total_injected += func.injected_interactions;
            total_hardcoded += func.hardcoded_interactions;
            let pressure = func.hardcoded_interactions as f64
                / (func.injected_interactions + func.hardcoded_interactions).max(1) as f64;
            edr_hotspots.push((
                format!("{}::{}", file.module_id, func.name),
                pressure,
                format!("{}:{}", file.module_id, func.line),
            ));
        }
    }
    let total_interactions = total_injected + total_hardcoded;
    let edr = if total_interactions == 0 {
        1.0
    } else {
        total_injected as f64 / total_interactions as f64
    };
    let edr_risk = risk_descending(edr, 0.70, 0.40);
    overall_risk = RiskLevel::max(overall_risk, edr_risk);
    dimensions.push(Dimension {
        id: "testability_pluggability".to_string(),
        metric: "EDR".to_string(),
        raw: json!(round3(edr)),
        risk: edr_risk,
    });
    edr_hotspots.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (entity, value, location) in edr_hotspots.into_iter().take(top_n) {
        hotspots.push(Hotspot {
            dimension_id: "testability_pluggability".to_string(),
            entity,
            metric_value: round3(value),
            location,
            reason: "Hardcoded dependency pressure".to_string(),
        });
    }

    // 6) intent_redundancy / PLME
    let mut plme_sum = 0usize;
    let mut unique_targets = HashSet::new();
    let mut plme_hotspots = Vec::new();
    for file in facts {
        for import in &file.imports {
            plme_sum += import.distance;
            unique_targets.insert(
                import
                    .resolved_target
                    .clone()
                    .unwrap_or_else(|| import.raw_target.clone()),
            );
            plme_hotspots.push((
                format!("{} -> {}", file.module_id, import.raw_target),
                import.distance as f64,
                file.module_id.clone(),
            ));
        }
    }
    let plme = plme_sum as f64 / unique_targets.len().max(1) as f64;
    let plme_risk = risk_ascending(plme, 0.40, 1.20);
    overall_risk = RiskLevel::max(overall_risk, plme_risk);
    dimensions.push(Dimension {
        id: "intent_redundancy".to_string(),
        metric: "PLME".to_string(),
        raw: json!(round3(plme)),
        risk: plme_risk,
    });
    plme_hotspots.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (entity, value, location) in plme_hotspots.into_iter().take(top_n) {
        hotspots.push(Hotspot {
            dimension_id: "intent_redundancy".to_string(),
            entity,
            metric_value: round3(value),
            location,
            reason: "Deep relative import path".to_string(),
        });
    }

    // 7) state_encapsulation / SSE + OA
    let declared_mutable: usize = facts.iter().map(|f| f.mutable_declared).sum();
    let mutated_mutable: usize = facts.iter().map(|f| f.mutable_mutated).sum();
    let sse = declared_mutable as f64 / mutated_mutable.max(1) as f64;
    let sse_risk = risk_ascending(sse, 1.10, 1.60);

    let mut writers: HashMap<String, HashSet<String>> = HashMap::new();
    let mut oa_hotspots = Vec::new();
    for file in facts {
        for write in &file.member_writes {
            writers
                .entry(write.entity.clone())
                .or_default()
                .insert(file.module_id.clone());
            oa_hotspots.push((
                write.entity.clone(),
                format!("{}:{}", file.module_id, write.line),
            ));
        }
    }

    let mut oa_entities = Vec::new();
    for (entity, writer_set) in &writers {
        let writer_count = writer_set.len();
        let value = if writer_count <= 1 {
            0.0
        } else {
            1.0 - (1.0 / writer_count as f64)
        };
        oa_entities.push((entity.clone(), value, writer_count));
    }

    let oa = if oa_entities.is_empty() {
        0.0
    } else {
        oa_entities.iter().map(|(_, v, _)| *v).sum::<f64>() / oa_entities.len() as f64
    };
    let oa_risk = risk_ascending(oa, 0.10, 0.35);
    let state_risk = RiskLevel::max(sse_risk, oa_risk);
    overall_risk = RiskLevel::max(overall_risk, state_risk);

    dimensions.push(Dimension {
        id: "state_encapsulation".to_string(),
        metric: "SSE+OA".to_string(),
        raw: json!({
            "sse": round3(sse),
            "oa": round3(oa)
        }),
        risk: state_risk,
    });

    let mut sse_hotspots = facts
        .iter()
        .map(|f| {
            (
                f.module_id.clone(),
                f.mutable_declared as f64 / f.mutable_mutated.max(1) as f64,
            )
        })
        .collect::<Vec<_>>();
    sse_hotspots.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (entity, value) in sse_hotspots.into_iter().take(top_n) {
        hotspots.push(Hotspot {
            dimension_id: "state_encapsulation".to_string(),
            entity,
            metric_value: round3(value),
            location: "file".to_string(),
            reason: "High mutable declaration expansion (SSE)".to_string(),
        });
    }

    oa_entities.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (entity, value, writer_count) in oa_entities.into_iter().take(top_n) {
        hotspots.push(Hotspot {
            dimension_id: "state_encapsulation".to_string(),
            entity,
            metric_value: round3(value),
            location: "entity".to_string(),
            reason: format!("Ownership ambiguity across {writer_count} writers (OA)"),
        });
    }

    // Keep ordering stable for output determinism.
    hotspots.sort_by(|a, b| {
        a.dimension_id
            .cmp(&b.dimension_id)
            .then_with(|| b.metric_value.total_cmp(&a.metric_value))
            .then_with(|| a.entity.cmp(&b.entity))
    });

    ComputedMetrics {
        dimensions,
        hotspots,
        overall_risk,
    }
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

fn percentile(mut values: Vec<f64>, p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    let p = p.clamp(0.0, 1.0);
    let idx = ((values.len() - 1) as f64 * p).round() as usize;
    values[idx]
}

fn risk_ascending(value: f64, low_max: f64, medium_max: f64) -> RiskLevel {
    if value <= low_max {
        RiskLevel::Low
    } else if value <= medium_max {
        RiskLevel::Medium
    } else {
        RiskLevel::High
    }
}

fn risk_descending(value: f64, low_min: f64, medium_min: f64) -> RiskLevel {
    if value >= low_min {
        RiskLevel::Low
    } else if value >= medium_min {
        RiskLevel::Medium
    } else {
        RiskLevel::High
    }
}

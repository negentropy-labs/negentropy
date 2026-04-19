use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::context::ProjectContext;
use crate::report::*;

use super::{Metric, MetricResult};

/// State Space Expansion (SSE) + Ownership Ambiguity (OA)
pub struct SseMetric;

impl Metric for SseMetric {
    fn id(&self) -> &str {
        "state_encapsulation"
    }

    fn analyze_project(&self, ctx: &ProjectContext, top_n: usize) -> MetricResult {
        let mut diagnostics = Vec::new();
        let mut hotspots = Vec::new();
        let mut total_declared = 0usize;
        let mut total_mutated = 0usize;

        let mut writer_map: HashMap<String, HashSet<String>> = HashMap::new();
        let mut file_scores = Vec::new();

        for fact in &ctx.facts {
            let declared = fact.mutable_declarations.len();
            let mutated = fact
                .mutable_declarations
                .iter()
                .filter(|decl| decl.mutated)
                .count();

            total_declared += declared;
            total_mutated += mutated;

            let sse_score = declared as f64 / mutated.max(1) as f64;
            file_scores.push((fact.file.clone(), sse_score));

            for decl in &fact.mutable_declarations {
                if decl.mutated {
                    continue;
                }

                diagnostics.push(Diagnostic {
                    id: format!("SSE-{:03}", diagnostics.len() + 1),
                    risk: RiskLevel::Low,
                    metric: "SSE".into(),
                    file: fact.file.clone(),
                    line: decl.line,
                    message: format!("`let {}` is never reassigned", decl.name),
                    suggestion: format!(
                        "Change `let {}` to `const {}` to narrow the state space",
                        decl.name, decl.name
                    ),
                });
            }

            for write in &fact.member_writes {
                writer_map
                    .entry(write.entity.clone())
                    .or_default()
                    .insert(fact.file.clone());
            }
        }

        let sse = total_declared as f64 / total_mutated.max(1) as f64;
        let sse_risk = risk_ascending(sse, 1.10, 1.60);

        file_scores.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (file, score) in file_scores.into_iter().take(top_n) {
            hotspots.push(Hotspot {
                dimension_id: self.id().into(),
                entity: file,
                metric_value: round3(score),
                location: "file".into(),
                reason: "High mutable declaration expansion (SSE)".into(),
            });
        }

        let mut oa_entities = writer_map
            .into_iter()
            .map(|(entity, writers)| {
                let writer_count = writers.len();
                let score = if writer_count <= 1 {
                    0.0
                } else {
                    1.0 - (1.0 / writer_count as f64)
                };
                (entity, score, writer_count)
            })
            .collect::<Vec<_>>();

        let oa = if oa_entities.is_empty() {
            0.0
        } else {
            oa_entities.iter().map(|(_, score, _)| *score).sum::<f64>() / oa_entities.len() as f64
        };
        let oa_risk = risk_ascending(oa, 0.10, 0.35);
        let risk = RiskLevel::max(sse_risk, oa_risk);

        oa_entities.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (entity, score, writer_count) in oa_entities.into_iter().take(top_n) {
            hotspots.push(Hotspot {
                dimension_id: self.id().into(),
                entity,
                metric_value: round3(score),
                location: "entity".into(),
                reason: format!("Ownership ambiguity across {writer_count} writers (OA)"),
            });
        }

        MetricResult {
            dimension: DimensionSummary {
                id: self.id().into(),
                metric: "SSE+OA".into(),
                raw: json!({
                    "sse": round3(sse),
                    "oa": round3(oa),
                }),
                risk,
            },
            hotspots,
            diagnostics,
        }
    }
}

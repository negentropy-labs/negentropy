use serde_json::json;

use crate::context::ProjectContext;
use crate::report::*;

use super::{Metric, MetricResult};

/// Topological Coupling Entropy (TCE)
pub struct TceMetric;

impl Metric for TceMetric {
    fn id(&self) -> &str {
        "architecture_decoupling"
    }

    fn analyze_project(&self, ctx: &ProjectContext, _top_n: usize) -> MetricResult {
        let mut diagnostics = Vec::new();

        for cycle in &ctx.graph.cycles {
            let file_list = cycle.join(" <-> ");
            for file in cycle {
                diagnostics.push(Diagnostic {
                    id: format!("TCE-{:03}", diagnostics.len() + 1),
                    risk: RiskLevel::High,
                    metric: "TCE".into(),
                    file: file.clone(),
                    line: 0,
                    message: format!(
                        "Circular dependency cycle ({} modules): {}",
                        cycle.len(),
                        file_list
                    ),
                    suggestion:
                        "Extract shared interface into a third module, or use event-based decoupling"
                            .into(),
                });
            }
        }

        let risk = risk_ascending(ctx.graph.tce, 0.10, 0.30);

        MetricResult {
            dimension: DimensionSummary {
                id: self.id().into(),
                metric: "TCE".into(),
                raw: json!(round3(ctx.graph.tce)),
                risk,
            },
            hotspots: Vec::new(),
            diagnostics,
        }
    }
}

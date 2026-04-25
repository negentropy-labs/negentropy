use serde_json::json;

use crate::context::ProjectContext;
use crate::model::Dimension;

use super::{metric_output, risk_ascending, round3};

pub(super) fn compute(context: &ProjectContext) -> super::MetricOutput {
    let risk = risk_ascending(context.graph.tce, 0.10, 0.30);

    metric_output(
        Dimension {
            id: "architecture_decoupling".to_string(),
            metric: "TCE".to_string(),
            raw: json!(round3(context.graph.tce)),
            risk,
        },
        Vec::new(),
    )
}

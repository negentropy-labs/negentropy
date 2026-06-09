mod bfp;
mod dis;
mod dmr;
mod ead;
mod edr;
mod iie;
mod ldp;
mod plme;
mod state;
mod tce;
mod tcr;
mod vnd;

use crate::context::ProjectContext;
use crate::model::{ComputedMetrics, Dimension, Hotspot, RiskLevel};

struct MetricOutput {
    dimension: Dimension,
    hotspots: Vec<Hotspot>,
}

pub fn compute_metrics(context: &ProjectContext, top_n: usize) -> ComputedMetrics {
    let outputs = vec![
        iie::compute(context, top_n),
        ead::compute(context, top_n),
        tcr::compute(context, top_n),
        tce::compute(context),
        edr::compute(context, top_n),
        plme::compute(context, top_n),
        state::compute(context, top_n),
        vnd::compute(context, top_n),
        ldp::compute(context, top_n),
        dis::compute(context, top_n),
        dmr::compute(context, top_n),
        bfp::compute(context, top_n),
    ];

    let mut dimensions = Vec::with_capacity(outputs.len());
    let mut hotspots = Vec::new();
    let mut overall_risk = RiskLevel::Low;

    for output in outputs {
        overall_risk = RiskLevel::max(overall_risk, output.dimension.risk);
        dimensions.push(output.dimension);
        hotspots.extend(output.hotspots);
    }

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

fn metric_output(dimension: Dimension, hotspots: Vec<Hotspot>) -> MetricOutput {
    MetricOutput {
        dimension,
        hotspots,
    }
}

fn positive_hotspots(mut hotspots: Vec<Hotspot>, top_n: usize) -> Vec<Hotspot> {
    hotspots.retain(|hotspot| hotspot.metric_value > 0.0);
    hotspots.sort_by(|a, b| b.metric_value.total_cmp(&a.metric_value));
    hotspots.truncate(top_n);
    hotspots
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

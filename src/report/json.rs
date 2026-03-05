use super::Report;

pub fn print_json(report: &Report) {
    let json = serde_json::to_string_pretty(report).expect("failed to serialize report");
    println!("{json}");
}

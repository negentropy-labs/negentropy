use std::path::Path;

use super::Report;

pub fn print_json(report: &Report) {
    let json = serde_json::to_string_pretty(report).expect("failed to serialize report");
    println!("{json}");
}

pub fn write_json(report: &Report, path: &Path) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(path, &json)?;
    Ok(())
}

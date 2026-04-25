#![allow(deprecated)]

use std::fs;

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn default_extensions_include_mts() {
    let dir = tempdir().expect("tempdir");
    fs::write(
        dir.path().join("sample.mts"),
        "export const value = 1;\nlet x = 0; x += 1;",
    )
    .expect("write mts");
    fs::write(dir.path().join("ignored.txt"), "hello").expect("write txt");

    let output = Command::new(cargo_bin("negentropy"))
        .args([
            "analyze",
            dir.path().to_str().expect("path"),
            "--format",
            "json",
            "--fail-on",
            "none",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["summary"]["files_scanned"], 1);

    let exts = json["effective_extensions"].as_array().expect("extensions");
    assert!(exts.iter().any(|v| v.as_str() == Some(".mts")));
}

#[test]
fn custom_extensions_filter_files() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("a.ts"), "export const a = 1;").expect("write ts");
    fs::write(dir.path().join("b.mts"), "export const b = 2;").expect("write mts");
    fs::write(dir.path().join("c.js"), "export const c = 3;").expect("write js");

    let output = Command::new(cargo_bin("negentropy"))
        .args([
            "analyze",
            dir.path().to_str().expect("path"),
            "--format",
            "json",
            "--fail-on",
            "none",
            "--extensions",
            ".ts,.mts",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["summary"]["files_scanned"], 2);

    let exts = json["effective_extensions"]
        .as_array()
        .expect("extensions")
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>();
    assert_eq!(exts, vec![".mts", ".ts"]);
}

#[test]
fn invalid_extension_exits_with_code_1() {
    let dir = tempdir().expect("tempdir");

    Command::new(cargo_bin("negentropy"))
        .args([
            "analyze",
            dir.path().to_str().expect("path"),
            "--extensions",
            "ts,.js",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("must start with '.'"));
}

#[test]
fn single_file_repo_has_no_graph_distortion_or_zero_hotspots() {
    let dir = tempdir().expect("tempdir");
    fs::write(
        dir.path().join("sample.ts"),
        "export function wrap(value) { return value + 1; }",
    )
    .expect("write sample");

    let output = Command::new(cargo_bin("negentropy"))
        .args([
            "analyze",
            dir.path().to_str().expect("path"),
            "--format",
            "json",
            "--fail-on",
            "none",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).expect("json");
    let dimensions = json["dimensions"].as_array().expect("dimensions");

    let tcr = dimensions
        .iter()
        .find(|dimension| dimension["id"].as_str() == Some("change_blast_radius"))
        .expect("tcr dimension");
    assert_eq!(tcr["raw"], serde_json::json!(0.0));
    assert_eq!(tcr["risk"], serde_json::json!("low"));

    let tce = dimensions
        .iter()
        .find(|dimension| dimension["id"].as_str() == Some("architecture_decoupling"))
        .expect("tce dimension");
    assert_eq!(tce["raw"], serde_json::json!(0.0));
    assert_eq!(tce["risk"], serde_json::json!("low"));

    let hotspots = json["hotspots"].as_array().expect("hotspots");
    assert!(
        hotspots
            .iter()
            .all(|hotspot| { hotspot["metric_value"].as_f64().expect("hotspot value") > 0.0 })
    );
}

#[test]
fn baseline_report_includes_dimension_and_hotspot_delta() {
    let baseline_dir = tempdir().expect("baseline tempdir");
    let baseline_path = baseline_dir.path().join("baseline.json");
    fs::write(
        baseline_dir.path().join("old.ts"),
        "export function oldThing(order) { return order.total + order.tax; }\n",
    )
    .expect("write baseline source");

    Command::new(cargo_bin("negentropy"))
        .args([
            "analyze",
            baseline_dir.path().to_str().expect("path"),
            "--format",
            "json",
            "--fail-on",
            "none",
            "--output",
            baseline_path.to_str().expect("baseline path"),
        ])
        .assert()
        .success();

    let current_dir = tempdir().expect("current tempdir");
    fs::write(
        current_dir.path().join("new.ts"),
        "export function newThing(order) { return order.id + order.total + order.tax; }\n",
    )
    .expect("write current source");

    let output = Command::new(cargo_bin("negentropy"))
        .args([
            "analyze",
            current_dir.path().to_str().expect("path"),
            "--format",
            "json",
            "--fail-on",
            "none",
            "--baseline",
            baseline_path.to_str().expect("baseline path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).expect("json");
    let delta = &json["delta"];

    let dimensions = delta["dimensions"].as_array().expect("dimension deltas");
    let logic_cohesion = dimensions
        .iter()
        .find(|dimension| dimension["id"].as_str() == Some("logic_cohesion"))
        .expect("logic cohesion delta");
    assert_eq!(logic_cohesion["raw_delta"], serde_json::json!(1.0));

    let new_hotspots = delta["new_hotspots"].as_array().expect("new hotspots");
    assert!(
        new_hotspots
            .iter()
            .any(|hotspot| hotspot["entity"].as_str() == Some("new.ts::newThing"))
    );

    let resolved_hotspots = delta["resolved_hotspots"]
        .as_array()
        .expect("resolved hotspots");
    assert!(
        resolved_hotspots
            .iter()
            .any(|hotspot| hotspot["entity"].as_str() == Some("old.ts::oldThing"))
    );
}

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

    let definitions = json["metric_definitions"]
        .as_array()
        .expect("metric definitions");
    assert!(definitions.iter().any(|definition| {
        definition["id"].as_str() == Some("behavior_mode_pressure")
            && definition["metric"].as_str() == Some("BFP")
            && definition["description"]
                .as_str()
                .is_some_and(|description| description.contains("boolean-like flags"))
    }));
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
fn typescript_and_tsx_parse_without_recovery_diagnostics() {
    let dir = tempdir().expect("tempdir");
    fs::write(
        dir.path().join("model.mts"),
        r#"
type Result<T> = Readonly<{ value: T }>;
interface Candidate {
  readonly id: string;
  active?: boolean;
}

const defaults = { active: true } as const satisfies Partial<Candidate>;

export function renderCandidate(candidate: Candidate): Result<string> {
  return { value: candidate.id };
}

setTestStates({ ready: true });
useSkipOnNonce("scope", "nonce", "fallback");
"#,
    )
    .expect("write mts");
    fs::write(
        dir.path().join("view.tsx"),
        r#"
import { renderCandidate } from "./model.mjs";

export function CandidateCard({ candidate }: { candidate: { id: string } }) {
  const title = renderCandidate(candidate).value;
  return <section data-testid="candidate-card">{title}</section>;
}
"#,
    )
    .expect("write tsx");

    let output = Command::new(cargo_bin("negentropy"))
        .args([
            "analyze",
            dir.path().to_str().expect("path"),
            "--extensions",
            ".mts,.tsx",
            "--format",
            "json",
            "--fail-on",
            "none",
            "--top",
            "20",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["summary"]["files_scanned"], 2);
    assert_eq!(json["summary"]["parsed_files"], 2);
    assert_eq!(json["summary"]["files_with_parse_errors"], 0);
    assert_eq!(
        json["parse_diagnostics"]
            .as_array()
            .expect("diagnostics")
            .len(),
        0
    );

    let hotspots = json["hotspots"].as_array().expect("hotspots");
    assert!(!hotspots.iter().any(|hotspot| {
        hotspot["dimension_id"].as_str() == Some("behavior_mode_pressure")
            && hotspot["entity"]
                .as_str()
                .is_some_and(|entity| entity.contains("renderCandidate"))
    }));
}

#[test]
fn parse_errors_emit_partial_report_and_skip_quality_gate() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("broken.ts"), "export const = ;\n").expect("write broken");

    let output = Command::new(cargo_bin("negentropy"))
        .args([
            "analyze",
            dir.path().to_str().expect("path"),
            "--format",
            "json",
            "--fail-on",
            "high",
        ])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).expect("json");
    assert_eq!(json["summary"]["files_scanned"], 1);
    assert_eq!(json["summary"]["parsed_files"], 0);
    assert_eq!(json["summary"]["files_with_parse_errors"], 1);

    let diagnostics = json["parse_diagnostics"].as_array().expect("diagnostics");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["path"], "broken.ts");
    assert_eq!(diagnostics[0]["language"], "typescript");
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

#[test]
fn taste_metrics_report_quantified_signals() {
    let dir = tempdir().expect("tempdir");
    fs::create_dir_all(dir.path().join("src/features/orders/deep")).expect("mkdir orders");
    fs::create_dir_all(dir.path().join("src/shared/common")).expect("mkdir shared");
    fs::create_dir_all(dir.path().join("src/features/payments/internal")).expect("mkdir payments");

    fs::write(
        dir.path().join("src/index.ts"),
        "export { renderOrder } from './features/orders/deep/manager';\n",
    )
    .expect("write index");
    fs::write(
        dir.path().join("src/shared/common/utils.ts"),
        r#"
export function formatThing(value: number, dryRun: boolean, force: boolean) {
  if (dryRun) return "pending";
  if (force) return "pending";
  return "complete";
}
"#,
    )
    .expect("write utils");
    fs::write(
        dir.path().join("src/features/payments/internal/secret.ts"),
        "export const secret = 42;\n",
    )
    .expect("write internal");
    fs::write(
        dir.path().join("src/features/orders/deep/manager.ts"),
        r#"
import { formatThing } from "../../../shared/common/utils";
import { secret } from "../../payments/internal/secret";

export function renderOrder(total: number) {
  return formatThing(total + secret, true, false);
}
"#,
    )
    .expect("write manager");
    fs::write(
        dir.path().join("src/features/orders/orphan.ts"),
        "export const unusedOrderMode = 'pending';\n",
    )
    .expect("write orphan");

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
    for id in [
        "naming_clarity",
        "literal_consolidation",
        "directory_alignment",
        "module_reachability",
        "behavior_mode_pressure",
    ] {
        let dimension = dimensions
            .iter()
            .find(|dimension| dimension["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("missing dimension {id}"));
        assert!(
            dimension["raw"].as_f64().expect("numeric raw") > 0.0,
            "expected positive raw score for {id}"
        );
    }

    let hotspots = json["hotspots"].as_array().expect("hotspots");
    for id in [
        "naming_clarity",
        "literal_consolidation",
        "directory_alignment",
        "module_reachability",
        "behavior_mode_pressure",
    ] {
        assert!(
            hotspots
                .iter()
                .any(|hotspot| hotspot["dimension_id"].as_str() == Some(id)),
            "expected hotspot for {id}"
        );
    }
}

#[test]
fn table_output_includes_metric_guide_in_stdout_and_output_file() {
    let dir = tempdir().expect("tempdir");
    let report_path = dir.path().join("report.txt");
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
            "table",
            "--fail-on",
            "none",
            "--output",
            report_path.to_str().expect("report path"),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("stdout utf8");
    assert!(stdout.contains("Metric Guide"));
    assert!(stdout.contains("behavior_mode_pressure | BFP"));

    let report = fs::read_to_string(report_path).expect("read report");
    assert!(report.contains("Metric Guide"));
    assert!(report.contains("naming_clarity | VND"));
}

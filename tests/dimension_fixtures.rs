#![allow(deprecated)]

use std::path::PathBuf;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;

fn risk_rank(risk: &str) -> u8 {
    match risk {
        "low" => 0,
        "medium" => 1,
        "high" => 2,
        other => panic!("unknown risk level: {other}"),
    }
}

fn analyze_dimension_risk(fixture_path: &str, dimension_id: &str) -> u8 {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = repo_root.join(fixture_path);

    let output = Command::new(cargo_bin("negentropy"))
        .args([
            "analyze",
            path.to_str().expect("path"),
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

    let json: serde_json::Value = serde_json::from_slice(&output).expect("valid json output");
    let dims = json["dimensions"].as_array().expect("dimensions array");
    let dim = dims
        .iter()
        .find(|d| d["id"].as_str() == Some(dimension_id))
        .expect("dimension exists");

    risk_rank(dim["risk"].as_str().expect("risk string"))
}

#[test]
fn dimension_fixtures_good_should_score_better_than_bad() {
    let cases = [
        (
            "module_abstraction",
            "tests/fixtures/dimensions/module_abstraction/good",
            "tests/fixtures/dimensions/module_abstraction/bad",
        ),
        (
            "logic_cohesion",
            "tests/fixtures/dimensions/logic_cohesion/good",
            "tests/fixtures/dimensions/logic_cohesion/bad",
        ),
        (
            "change_blast_radius",
            "tests/fixtures/dimensions/change_blast_radius/good",
            "tests/fixtures/dimensions/change_blast_radius/bad",
        ),
        (
            "architecture_decoupling",
            "tests/fixtures/dimensions/architecture_decoupling/good",
            "tests/fixtures/dimensions/architecture_decoupling/bad",
        ),
        (
            "testability_pluggability",
            "tests/fixtures/dimensions/testability_pluggability/good",
            "tests/fixtures/dimensions/testability_pluggability/bad",
        ),
        (
            "intent_redundancy",
            "tests/fixtures/dimensions/intent_redundancy/good",
            "tests/fixtures/dimensions/intent_redundancy/bad",
        ),
        (
            "state_encapsulation",
            "tests/fixtures/dimensions/state_encapsulation/good",
            "tests/fixtures/dimensions/state_encapsulation/bad",
        ),
    ];

    for (dimension, good_path, bad_path) in cases {
        let good = analyze_dimension_risk(good_path, dimension);
        let bad = analyze_dimension_risk(bad_path, dimension);
        assert!(
            good < bad,
            "expected good fixture to have strictly lower risk for {dimension}, got good={good}, bad={bad}"
        );
    }
}

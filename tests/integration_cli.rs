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

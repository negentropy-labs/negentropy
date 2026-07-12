use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    pub scan: ScanConfig,
    pub privacy: PrivacyConfig,
}

impl ProjectConfig {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("negentropy.toml");
        if !path.exists() {
            return Ok(Self::default());
        }

        let data = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        toml::from_str(&data).with_context(|| format!("failed to parse config {}", path.display()))
    }

    pub fn digest(&self) -> Result<String> {
        let data = serde_json::to_vec(self)?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        Ok(format!("{:x}", hasher.finalize()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanConfig {
    pub extensions: Option<Vec<String>>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub include_generated: bool,
    pub include_tests: bool,
    pub include_migrations: bool,
    pub include_benches: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            extensions: None,
            include: Vec::new(),
            exclude: Vec::new(),
            include_generated: true,
            include_tests: true,
            include_migrations: true,
            include_benches: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacyConfig {
    pub literal_payload: LiteralPayloadMode,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            literal_payload: LiteralPayloadMode::Full,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LiteralPayloadMode {
    #[default]
    Full,
    Redacted,
    None,
}

pub fn is_test_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("tests/")
        || lower.contains("__tests__/")
        || lower.contains("/tests/")
        || lower.contains(".test.")
        || lower.contains(".spec.")
        || lower.ends_with("_test.ts")
        || lower.ends_with("_test.js")
}

pub fn is_generated_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("generated/")
        || lower.starts_with("gen/")
        || lower.contains("/generated/")
        || lower.contains(".generated.")
        || lower.contains("/gen/")
        || lower.ends_with(".gen.ts")
        || lower.ends_with(".gen.js")
}

pub fn is_migration_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("migrations/")
        || lower.starts_with("migration/")
        || lower.contains("/migrations/")
        || lower.contains("/migration/")
        || lower.contains(".migration.")
        || lower.contains("_migration.")
}

pub fn is_bench_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("bench/")
        || lower.starts_with("benches/")
        || lower.contains("/bench/")
        || lower.contains("/benches/")
        || lower.contains(".bench.")
        || lower.contains(".benchmark.")
}

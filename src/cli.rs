use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand, ValueEnum};

pub const DEFAULT_EXTENSIONS: &str = ".ts,.tsx,.js,.jsx,.mjs,.cjs,.mts";

#[derive(Debug, Parser)]
#[command(
    name = "negentropy",
    version,
    about = "V2 entropy analysis for TS/JS repos"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Analyze(AnalyzeArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Json,
    Table,
    Both,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FailOn {
    None,
    Medium,
    High,
}

#[derive(Debug, clap::Args)]
pub struct AnalyzeArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,

    #[arg(long, value_enum, default_value = "both")]
    pub format: OutputFormat,

    #[arg(long)]
    pub output: Option<PathBuf>,

    #[arg(long, default_value_t = 3)]
    pub top: usize,

    #[arg(long, value_enum, default_value = "high")]
    pub fail_on: FailOn,

    #[arg(long, help = "CSV list, e.g. .ts,.tsx,.mts")]
    pub extensions: Option<String>,
}

impl AnalyzeArgs {
    pub fn effective_extensions(&self) -> Result<Vec<String>> {
        normalize_extensions(self.extensions.as_deref())
    }
}

pub fn normalize_extensions(input: Option<&str>) -> Result<Vec<String>> {
    let raw = input.unwrap_or(DEFAULT_EXTENSIONS);
    let mut set = BTreeSet::new();

    for part in raw.split(',') {
        let ext = part.trim().to_ascii_lowercase();
        if ext.is_empty() {
            continue;
        }
        if !ext.starts_with('.') || ext.len() < 2 {
            return Err(anyhow!(
                "invalid extension '{ext}': each extension must start with '.'"
            ));
        }
        set.insert(ext);
    }

    if set.is_empty() {
        return Err(anyhow!("no valid extensions provided"));
    }

    Ok(set.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::normalize_extensions;

    #[test]
    fn normalizes_extensions() {
        let exts = normalize_extensions(Some(".TS, .mts,.ts")).expect("normalize");
        assert_eq!(exts, vec![".mts".to_string(), ".ts".to_string()]);
    }

    #[test]
    fn rejects_invalid_extension() {
        let err = normalize_extensions(Some("ts,.js")).expect_err("invalid");
        assert!(err.to_string().contains("must start with '.'"));
    }
}

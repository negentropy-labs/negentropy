use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::{DirEntry, WalkBuilder};

use crate::config::{
    ScanConfig, is_bench_path, is_generated_path, is_migration_path, is_test_path,
};

fn should_descend(entry: &DirEntry) -> bool {
    let Some(file_type) = entry.file_type() else {
        return true;
    };
    if !file_type.is_dir() {
        return true;
    }

    let name = entry.file_name().to_string_lossy();
    !(name == ".git" || name == "node_modules" || name == "dist" || name == "build")
}

pub fn discover_files(
    root: &Path,
    extensions: &[String],
    scan_config: &ScanConfig,
) -> Result<Vec<PathBuf>> {
    let include = globset(&scan_config.include)?;
    let mut exclude_patterns = scan_config.exclude.clone();
    exclude_patterns.extend(root_gitignore_patterns(root)?);
    let exclude = globset(&exclude_patterns)?;
    let mut files = Vec::new();
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false).filter_entry(should_descend);

    for entry in builder.build() {
        let entry = entry.with_context(|| format!("failed to walk {}", root.display()))?;
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }

        let path = entry.path();
        let relative_path = relative_path(path, root);
        if !accepts_scope(&relative_path, scan_config, &include, &exclude) {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_ascii_lowercase()));

        if let Some(ext) = ext
            && extensions.contains(&ext)
        {
            files.push(path.to_path_buf());
        }
    }

    files.sort();
    Ok(files)
}

fn accepts_scope(
    relative_path: &str,
    scan_config: &ScanConfig,
    include: &GlobSet,
    exclude: &GlobSet,
) -> bool {
    if !scan_config.include.is_empty() && !include.is_match(relative_path) {
        return false;
    }
    if exclude.is_match(relative_path) {
        return false;
    }
    if !scan_config.include_generated && is_generated_path(relative_path) {
        return false;
    }
    if !scan_config.include_tests && is_test_path(relative_path) {
        return false;
    }
    if !scan_config.include_migrations && is_migration_path(relative_path) {
        return false;
    }
    if !scan_config.include_benches && is_bench_path(relative_path) {
        return false;
    }

    true
}

fn globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    Ok(builder.build()?)
}

fn root_gitignore_patterns(root: &Path) -> Result<Vec<String>> {
    let path = root.join(".gitignore");
    if !path.exists() {
        return Ok(Vec::new());
    }

    let data = fs::read_to_string(&path)
        .with_context(|| format!("failed to read gitignore {}", path.display()))?;
    let mut patterns = Vec::new();
    for line in data.lines() {
        let pattern = line.trim();
        if pattern.is_empty() || pattern.starts_with('#') || pattern.starts_with('!') {
            continue;
        }

        patterns.extend(expand_gitignore_pattern(pattern));
    }

    Ok(patterns)
}

fn expand_gitignore_pattern(pattern: &str) -> Vec<String> {
    let normalized = pattern.trim_start_matches('/').trim_end_matches('/');
    if normalized.is_empty() {
        return Vec::new();
    }

    if pattern.ends_with('/') {
        vec![format!("{normalized}/**"), format!("**/{normalized}/**")]
    } else if normalized.contains('/') {
        vec![normalized.to_string()]
    } else {
        vec![normalized.to_string(), format!("**/{normalized}")]
    }
}

fn relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Debug, Deserialize)]
pub struct Dish {
    pub path: PathBuf,
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct Table {
    pub dishes: Vec<Dish>,
}

pub fn resolve_tool(name: &str, bin_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(dir) = bin_dir {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // Search PATH
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn parse_table(path: &Path) -> Result<Table> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("reading table.json at {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("parsing table.json at {}", path.display()))
}

pub fn dedup_check(ledger: &Path, artifact: &Path, date: &str) -> bool {
    let Ok(content) = std::fs::read_to_string(ledger) else {
        return false;
    };
    let artifact_str = artifact.to_string_lossy();
    for line in content.lines() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            let matches_artifact = val.get("artifact")
                .and_then(|v| v.as_str())
                .map(|s| s == artifact_str)
                .unwrap_or(false);
            let matches_date = val.get("date")
                .and_then(|v| v.as_str())
                .map(|s| s == date)
                .unwrap_or(false);
            if matches_artifact && matches_date {
                return true;
            }
        }
    }
    false
}

pub fn run_stage(cmd: &[&str], dry_run: bool) -> Result<Output> {
    if dry_run {
        println!("[dry-run] {}", cmd.join(" "));
        // Return a fake success output
        let output = Command::new("true").output().context("running 'true'")?;
        return Ok(output);
    }
    let (prog, args) = cmd.split_first().context("empty command")?;
    let output = Command::new(prog)
        .args(args)
        .output()
        .with_context(|| format!("spawning {prog}"))?;
    Ok(output)
}

//! Inspect the vicious-circle ledger for dedup purposes.
//!
//! The ledger is an append-only JSONL file. Each entry has at minimum
//! an `artifact_path` and a `date` field (YYYY-MM-DD). We use these to
//! determine whether an artifact has already been critiqued for a given date.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// A single ledger entry (only the fields we care about).
#[derive(Debug, Deserialize)]
pub struct LedgerEntry {
    /// Path to the artifact that was critiqued.
    pub artifact_path: Option<PathBuf>,
    /// ISO date (YYYY-MM-DD) of the session.
    pub date: Option<String>,
    /// The crowned bon mot for this entry, if any.
    pub crowned: Option<String>,
}

/// Return the set of artifact paths already recorded in the ledger for `date`.
///
/// # Errors
/// Returns an error if the ledger file exists but cannot be read or parsed.
pub fn already_recorded(ledger_path: &Path, date: &str) -> Result<HashSet<PathBuf>> {
    if !ledger_path.exists() {
        return Ok(HashSet::new());
    }
    let contents = std::fs::read_to_string(ledger_path)
        .with_context(|| format!("reading ledger at {}", ledger_path.display()))?;
    let mut recorded = HashSet::new();
    for (lineno, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<LedgerEntry>(line) {
            Ok(entry) => {
                if entry.date.as_deref() == Some(date) {
                    if let Some(path) = entry.artifact_path {
                        let _ = recorded.insert(path);
                    }
                }
            }
            Err(e) => {
                // Tolerate malformed lines — log and continue.
                eprintln!("roundtable: ledger line {lineno} parse error (skipping): {e}");
            }
        }
    }
    Ok(recorded)
}

/// Read the last crowned bon mot from the ledger for the given date.
///
/// Returns `None` if the ledger is empty, absent, or has no crowned entries for the date.
///
/// # Errors
/// Returns an error if the ledger file exists but cannot be read.
pub fn last_crowned(ledger_path: &Path, date: &str) -> Result<Option<String>> {
    if !ledger_path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(ledger_path)
        .with_context(|| format!("reading ledger at {}", ledger_path.display()))?;
    let mut last = None;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<LedgerEntry>(line) {
            if entry.date.as_deref() == Some(date) {
                if let Some(crowned) = entry.crowned {
                    last = Some(crowned);
                }
            }
        }
    }
    Ok(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_tmp(contents: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("tmp");
        f.write_all(contents.as_bytes()).expect("write");
        f
    }

    #[test]
    fn absent_ledger_returns_empty() {
        let result = already_recorded(Path::new("/tmp/no-such-ledger-xyz.jsonl"), "2026-06-15");
        assert!(result.expect("ok").is_empty());
    }

    #[test]
    fn dedup_finds_existing_entry() {
        let contents = r#"{"date":"2026-06-15","artifact_path":"/tmp/a.txt","crowned":"test"}
{"date":"2026-06-14","artifact_path":"/tmp/a.txt","crowned":"old"}
"#;
        let f = write_tmp(contents);
        let recorded = already_recorded(f.path(), "2026-06-15").expect("ok");
        assert!(recorded.contains(Path::new("/tmp/a.txt")));
        assert_eq!(recorded.len(), 1);
    }

    #[test]
    fn last_crowned_picks_latest() {
        let contents = r#"{"date":"2026-06-15","artifact_path":"/tmp/a.txt","crowned":"first bon mot"}
{"date":"2026-06-15","artifact_path":"/tmp/b.txt","crowned":"second bon mot"}
"#;
        let f = write_tmp(contents);
        let crowned = last_crowned(f.path(), "2026-06-15").expect("ok");
        assert_eq!(crowned.as_deref(), Some("second bon mot"));
    }
}

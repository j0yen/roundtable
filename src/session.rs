//! `roundtable session` — the full daily creative chain.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use anyhow::{bail, Context, Result};
use clap::Args;

use crate::{ledger, table};

/// Arguments for `roundtable session`.
#[derive(Debug, Args)]
pub struct SessionArgs {
    /// Date to run the session for (YYYY-MM-DD). Defaults to today.
    #[arg(long)]
    pub date: Option<String>,

    /// Print the stage plan without executing mutating stages.
    #[arg(long)]
    pub dry_run: bool,

    /// Directory containing the required tool binaries.
    /// Overrides PATH lookup for `the-lunch`, `vicious-circle`, `conning-tower`.
    #[arg(long)]
    pub bin_dir: Option<PathBuf>,

    /// Path to the vicious-circle ledger JSONL.
    /// Defaults to $XDG_DATA_HOME/vicious-circle/ledger.jsonl.
    #[arg(long, env = "ROUNDTABLE_LEDGER")]
    pub ledger: Option<PathBuf>,

    /// Directory for the canonical columns slot (passed to conning-tower syndicate).
    #[arg(long)]
    pub columns_dir: Option<PathBuf>,
}

/// Tool names that `roundtable session` requires.
const REQUIRED_TOOLS: &[&str] = &["the-lunch", "vicious-circle", "conning-tower"];

/// Resolve a binary name to its full path, preferring `--bin-dir` if set.
fn resolve_bin(name: &str, bin_dir: Option<&Path>) -> Result<PathBuf> {
    // Per-tool env override: ROUNDTABLE_LUNCH_BIN, ROUNDTABLE_VICIOUS_CIRCLE_BIN, etc.
    let env_key = format!(
        "ROUNDTABLE_{}_BIN",
        name.to_uppercase().replace('-', "_")
    );
    if let Ok(val) = std::env::var(&env_key) {
        let p = PathBuf::from(val);
        if p.exists() {
            return Ok(p);
        }
    }

    if let Some(dir) = bin_dir {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
        // Not found in --bin-dir and no PATH fallback requested.
        bail!(
            "required binary `{}` not found in --bin-dir `{}`",
            name,
            dir.display()
        );
    }

    // Search PATH.
    which_bin(name)
}

/// Look up `name` on PATH, returning the full path or a descriptive error.
fn which_bin(name: &str) -> Result<PathBuf> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    for dir in path_var.split(':') {
        let candidate = PathBuf::from(dir).join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    bail!(
        "required binary `{}` not found on PATH; install it or use --bin-dir",
        name
    );
}

/// Default ledger path: $XDG_DATA_HOME/vicious-circle/ledger.jsonl
fn default_ledger() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".local").join("share")
        });
    base.join("vicious-circle").join("ledger.jsonl")
}

/// State directory for the-lunch output: $XDG_STATE_HOME/the-lunch/<date>/
fn lunch_state_dir(date: &str) -> PathBuf {
    let base = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".local").join("state")
        });
    base.join("the-lunch").join(date)
}

/// Run a subprocess and return its exit status, or an error if it couldn't start.
fn run_cmd(cmd: &mut Command, stage_label: &str) -> Result<ExitStatus> {
    cmd.status()
        .with_context(|| format!("failed to start stage `{stage_label}`"))
}

/// Run `roundtable session`.
///
/// # Errors
/// Returns an error if any required binary is missing, or a stage fails.
#[allow(clippy::too_many_lines)]
pub fn run(args: SessionArgs) -> Result<()> {
    let date = args.date.unwrap_or_else(|| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Simple date computation: seconds since epoch → YYYY-MM-DD.
        epoch_to_date(now)
    });

    let ledger_path = args.ledger.unwrap_or_else(default_ledger);
    let bin_dir = args.bin_dir.as_deref();
    let dry_run = args.dry_run;

    // --- Phase 0: Resolve all required binaries upfront. ---
    let mut bins = std::collections::HashMap::new();
    for &tool in REQUIRED_TOOLS {
        match resolve_bin(tool, bin_dir) {
            Ok(path) => {
                let _ = bins.insert(tool, path);
            }
            Err(e) => {
                bail!("{e}");
            }
        }
    }

    let lunch_bin = &bins["the-lunch"];
    let vc_bin = &bins["vicious-circle"];
    let ct_bin = &bins["conning-tower"];

    // --- Phase 1: Set the table (the-lunch lunch). ---
    let lunch_stage = format!("the-lunch lunch --date {date}");
    if dry_run {
        println!("[dry-run] stage: lunch");
        println!("  command: {} lunch --date {date}", lunch_bin.display());
    } else {
        let status = run_cmd(
            Command::new(lunch_bin).args(["lunch", "--date", &date]),
            "lunch",
        )?;
        if !status.success() {
            bail!(
                "stage `lunch` failed (exit {:?}): {} lunch --date {}",
                status.code(),
                lunch_bin.display(),
                date
            );
        }
    }
    if !dry_run {
        println!("✓ lunch: table set for {date}");
    }

    // --- Phase 2: Read table.json to enumerate artifacts. ---
    let state_dir = lunch_state_dir(&date);
    let table_path = state_dir.join("table.json");

    // In dry-run mode we may not have a real table; just print the plan.
    if dry_run {
        println!("[dry-run] stage: read table from {}", table_path.display());
        println!("[dry-run] stage: for each artifact: vicious-circle record <artifact> --ledger {}", ledger_path.display());
        println!("[dry-run] stage: conning-tower compose --ledger {} --date {date}", ledger_path.display());
        // columns-dir for syndicate
        let columns_dir_display = args
            .columns_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<--columns-dir required>".into());
        println!(
            "[dry-run] stage: conning-tower syndicate --ledger {} --date {date} --to columns --columns-dir {columns_dir_display}",
            ledger_path.display()
        );
        println!("[dry-run] {} plan printed; no files were written.", lunch_stage);
        return Ok(());
    }

    let lunch_table = table::Table::from_path(&table_path)
        .with_context(|| format!("reading table.json after `the-lunch lunch --date {date}`"))?;

    let artifacts = lunch_table.present_artifacts();

    if artifacts.is_empty() {
        println!("no artifacts on the table today — nothing to critique. Session complete.");
        return Ok(());
    }

    // --- Phase 3: vicious-circle record for each artifact (with dedup). ---
    let already_done = ledger::already_recorded(&ledger_path, &date)
        .context("checking ledger for already-recorded artifacts")?;

    let mut critiqued_count: usize = 0;
    for artifact in &artifacts {
        if already_done.contains(*artifact) {
            println!("  (skipping {}: already in ledger for {date})", artifact.display());
            critiqued_count += 1; // counts toward total; it was done previously
            continue;
        }

        let stage_label = format!("critique:{}", artifact.display());
        let status = run_cmd(
            Command::new(vc_bin)
                .args(["record"])
                .arg(artifact.as_os_str())
                .args(["--ledger"])
                .arg(&ledger_path),
            &stage_label,
        )?;
        if !status.success() {
            bail!(
                "stage `{stage_label}` failed (exit {:?})",
                status.code()
            );
        }
        critiqued_count += 1;
        println!("  ✓ critiqued: {}", artifact.display());
    }

    // --- Phase 4: conning-tower compose. ---
    let compose_status = run_cmd(
        Command::new(ct_bin)
            .args(["compose", "--ledger"])
            .arg(&ledger_path)
            .args(["--date", &date]),
        "compose",
    )?;
    if !compose_status.success() {
        bail!(
            "stage `compose` failed (exit {:?})",
            compose_status.code()
        );
    }

    // --- Phase 5: conning-tower syndicate. ---
    let mut syndicate_cmd = Command::new(ct_bin);
    syndicate_cmd
        .args(["syndicate", "--ledger"])
        .arg(&ledger_path)
        .args(["--date", &date, "--to", "columns"]);
    if let Some(columns_dir) = &args.columns_dir {
        syndicate_cmd.args(["--columns-dir"]).arg(columns_dir);
    }
    let syndicate_status = run_cmd(&mut syndicate_cmd, "syndicate")?;
    if !syndicate_status.success() {
        bail!(
            "stage `syndicate` failed (exit {:?})",
            syndicate_status.code()
        );
    }

    // --- Summary. ---
    let crowned = ledger::last_crowned(&ledger_path, &date)
        .context("reading crowned bon mot from ledger")?
        .unwrap_or_else(|| "(none)".into());
    let column_dest = args
        .columns_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "columns slot".into());

    println!("\nSession complete for {date}:");
    println!("  {critiqued_count} artifact(s) critiqued");
    println!("  Crowned bon mot: {crowned}");
    println!("  Column → {column_dest}");

    Ok(())
}

/// Convert seconds since UNIX epoch to a YYYY-MM-DD string.
/// Uses only integer arithmetic; no chrono dependency in the binary.
fn epoch_to_date(secs: u64) -> String {
    // Days since epoch
    let days = secs / 86400;
    let mut remaining = days;

    let mut year: u64 = 1970;
    loop {
        let days_in_year: u64 = if is_leap(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }

    let leap = is_leap(year);
    let month_days: &[u64] = if leap {
        &[31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        &[31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month: u64 = 1;
    for &md in month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        month += 1;
    }
    let day = remaining + 1;
    format!("{year:04}-{month:02}-{day:02}")
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_to_date_known_values() {
        // 2026-06-15 = day 20619 since epoch
        // Verify a known date
        assert_eq!(epoch_to_date(0), "1970-01-01");
        // 2000-01-01: 10957 days * 86400
        assert_eq!(epoch_to_date(10_957 * 86400), "2000-01-01");
    }

    #[test]
    fn missing_bin_dir_tool_fails() {
        let tmp = tempfile::tempdir().expect("tmp");
        // A directory with no binaries
        let result = resolve_bin("the-lunch", Some(tmp.path()));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("the-lunch"), "message should name the tool: {msg}");
    }

    #[test]
    fn resolve_bin_from_bin_dir() {
        let tmp = tempfile::tempdir().expect("tmp");
        let fake_bin = tmp.path().join("fake-tool");
        std::fs::write(&fake_bin, "#!/bin/sh\necho hi").expect("write");
        let result = resolve_bin("fake-tool", Some(tmp.path()));
        assert!(result.is_ok(), "should find binary in --bin-dir");
        assert_eq!(result.expect("ok"), fake_bin);
    }
}

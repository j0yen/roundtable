use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
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

// ---------------------------------------------------------------------------
// Digest: read yesterday's crowned bon mot from the vicious-circle ledger
// ---------------------------------------------------------------------------

/// Output shape for `roundtable digest --format json`.
#[derive(Debug, Serialize)]
pub struct DigestJson {
    pub date: String,
    pub bon_mot: Option<BonMotSummary>,
    pub column_headline: Option<String>,
    pub fallback: bool,
}

#[derive(Debug, Serialize)]
pub struct BonMotSummary {
    pub line: String,
    pub author: String,
}

/// Read the most recent crowned bon mot for `date` from the vicious-circle
/// ledger JSONL at `ledger_path`.
///
/// The ledger contains one JSON object per line; each object may have a
/// `bon_mot` field (or `bon_mot: null` when no crown was reached).
/// We return the *last* crowned entry matching `date` (last-writer wins in
/// case of reruns).
pub fn read_bon_mot_for_date(ledger_path: &Path, date: &str) -> Option<(String, String)> {
    let content = std::fs::read_to_string(ledger_path).ok()?;
    let mut best: Option<(String, String)> = None;
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let entry_date = v.get("date").and_then(|d| d.as_str()).unwrap_or("");
        if entry_date != date {
            continue;
        }
        if let Some(bm) = v.get("bon_mot") {
            if bm.is_null() {
                continue;
            }
            let line_text = bm
                .get("line")
                .and_then(|l| l.as_str())
                .unwrap_or("")
                .to_string();
            let author = bm
                .get("author")
                .and_then(|a| a.as_str())
                .unwrap_or("")
                .to_string();
            if !line_text.is_empty() {
                best = Some((line_text, author));
            }
        }
    }
    best
}

/// Read the first headline from a conning-tower column markdown file.
///
/// The column file starts with `# <headline>` on its first non-blank line.
pub fn read_column_headline(column_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(column_path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            return Some(rest.trim().to_string());
        }
        // Stop at the first non-blank line even if it's not a heading
        return Some(trimmed.to_string());
    }
    None
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

// ---------------------------------------------------------------------------
// PRD 1: roundtable-bind
// ---------------------------------------------------------------------------

/// Scan JSONL files in `ledger_dir` for entries where the "date" field >=
/// `since_date`. Returns the "path" field as PathBuf for matching entries.
/// If `ledger_dir` doesn't exist, returns empty vec.
pub fn find_columns_since(ledger_dir: &Path, since_date: &str) -> Vec<PathBuf> {
    if !ledger_dir.exists() {
        return vec![];
    }
    let mut results = Vec::new();
    let entries = match std::fs::read_dir(ledger_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                let date_matches = val
                    .get("date")
                    .and_then(|d| d.as_str())
                    .map(|d| d >= since_date)
                    .unwrap_or(false);
                if date_matches {
                    if let Some(p) = val.get("path").and_then(|v| v.as_str()) {
                        results.push(PathBuf::from(p));
                    }
                }
            }
        }
    }
    results
}

/// Read `issues_dir` for the most recent issue date.
/// Issues are stored as subdirectories (or files) named YYYY-MM-DD.
/// Returns the lexicographically greatest name. If missing, returns None.
pub fn last_issue_date(issues_dir: &Path) -> Option<String> {
    if !issues_dir.exists() {
        return None;
    }
    let entries = std::fs::read_dir(issues_dir).ok()?;
    let mut dates: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            // Accept YYYY-MM-DD pattern (10 chars, digits and dashes)
            if name.len() == 10 && name.chars().all(|c| c.is_ascii_digit() || c == '-') {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    dates.sort();
    dates.into_iter().last()
}

// ---------------------------------------------------------------------------
// PRD 2: roundtable-games
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Opponent {
    Wordsmith,
    Pedant,
    Contrarian,
}

impl std::fmt::Display for Opponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Opponent::Wordsmith => write!(f, "Wordsmith"),
            Opponent::Pedant => write!(f, "Pedant"),
            Opponent::Contrarian => write!(f, "Contrarian"),
        }
    }
}

pub struct Game {
    pub article: PathBuf,
    pub opponent: Opponent,
    pub rounds: u8,
}

/// Run a game. Returns a transcript string.
/// In dry_run mode, returns the plan string without executing.
/// Otherwise: if `thanatopsis` is on PATH, shells to it; else produces a stub transcript.
pub fn run_game(game: &Game, dry_run: bool) -> String {
    if dry_run {
        return format!(
            "would start game: {} vs {}, {} rounds",
            game.article.display(),
            game.opponent,
            game.rounds
        );
    }
    // Check if thanatopsis is on PATH
    if resolve_tool("thanatopsis", None).is_some() {
        let opponent_str = game.opponent.to_string().to_lowercase();
        let rounds_str = game.rounds.to_string();
        let result = Command::new("thanatopsis")
            .args([
                "play",
                game.article.to_str().unwrap_or(""),
                "--opponent",
                &opponent_str,
                "--rounds",
                &rounds_str,
            ])
            .output();
        match result {
            Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
            Err(e) => format!("thanatopsis error: {e}"),
        }
    } else {
        // Stub transcript
        let mut transcript = String::new();
        for i in 1..=game.rounds {
            transcript.push_str(&format!("## Round {i}\n[stub repartee]\n"));
        }
        transcript
    }
}

// ---------------------------------------------------------------------------
// PRD 3: roundtable-digest (weekly)
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct WeeklyDigest {
    pub week: String,
    pub columns: Vec<String>,
    pub bon_mots: Vec<String>,
    pub games: Vec<String>,
    pub issue_date: Option<String>,
}

/// Compute ISO week string ("YYYY-Www") from a YYYY-MM-DD date string.
/// Uses `date` command to avoid adding chrono dep.
pub fn iso_week_from_date(date: &str) -> Option<String> {
    let output = Command::new("date")
        .args(["-d", date, "+%G-W%V"])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Get the current ISO week string via `date +%G-W%V`.
pub fn current_iso_week() -> String {
    Command::new("date")
        .arg("+%G-W%V")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "1970-W01".to_string())
}

/// Build a weekly digest from store_dir.
///
/// - `store_dir/ledger/` JSONL files: entries whose date falls in `week` →
///   `columns` (from "title" or "path" field) and `bon_mots` (from "crowned" field)
/// - `store_dir/games/` directory: files whose name contains the week → `games`
/// - `store_dir/issues/` directory: entries in that week → `issue_date`
pub fn build_digest(store_dir: &Path, week: &str) -> WeeklyDigest {
    if !store_dir.exists() {
        return WeeklyDigest {
            week: week.to_string(),
            ..Default::default()
        };
    }

    let mut digest = WeeklyDigest {
        week: week.to_string(),
        ..Default::default()
    };

    // Scan ledger dir
    let ledger_dir = store_dir.join("ledger");
    if ledger_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&ledger_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                for line in content.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                        // Check if entry date is in the requested week
                        let in_week = val
                            .get("date")
                            .and_then(|d| d.as_str())
                            .and_then(iso_week_from_date)
                            .map(|w| w == week)
                            .unwrap_or(false);
                        if !in_week {
                            continue;
                        }
                        // Collect column ref (prefer title, fall back to path)
                        let col_ref = val
                            .get("title")
                            .and_then(|v| v.as_str())
                            .or_else(|| val.get("path").and_then(|v| v.as_str()))
                            .map(|s| s.to_string());
                        if let Some(col) = col_ref {
                            digest.columns.push(col);
                        }
                        // Collect bon mot
                        if let Some(crowned) = val.get("crowned").and_then(|v| v.as_str()) {
                            if !crowned.is_empty() {
                                digest.bon_mots.push(crowned.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Scan games dir
    let games_dir = store_dir.join("games");
    if games_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&games_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains(week) {
                    digest.games.push(name);
                }
            }
        }
    }

    // Check issues dir for an issue in this week
    let issues_dir = store_dir.join("issues");
    if issues_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&issues_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                // Check if this issue date falls in our week
                let in_week = iso_week_from_date(&name)
                    .map(|w| w == week)
                    .unwrap_or(false);
                if in_week {
                    digest.issue_date = Some(name);
                    break;
                }
            }
        }
    }

    digest
}

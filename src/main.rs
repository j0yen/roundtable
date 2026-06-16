use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use roundtable::{dedup_check, parse_table, resolve_tool, run_stage};
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    sigpipe::reset();
    let cli = Cli::parse();
    match cli.command {
        Commands::Session(args) => run_session(args),
    }
}

#[derive(Parser)]
#[command(name = "roundtable", about = "Daily session orchestrator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Convene the full daily session: the-lunch → vicious-circle → conning-tower
    Session(SessionArgs),
}

#[derive(Parser)]
struct SessionArgs {
    /// Date in YYYY-MM-DD format (default: today)
    #[arg(long)]
    date: Option<String>,

    /// Print stage commands without executing mutating stages
    #[arg(long)]
    dry_run: bool,

    /// Directory to search for tool binaries (overrides PATH)
    #[arg(long)]
    bin_dir: Option<PathBuf>,
}

fn today_date() -> Result<String> {
    let output = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .context("getting today's date")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_session(args: SessionArgs) -> Result<()> {
    let date = match args.date {
        Some(d) => d,
        None => today_date()?,
    };
    let bin_dir = args.bin_dir.as_deref();
    let dry_run = args.dry_run;

    // Resolve tools (env var overrides)
    let lunch_bin = resolve_bin("the-lunch", "ROUNDTABLE_LUNCH_BIN", bin_dir)?;
    let circle_bin = resolve_bin("vicious-circle", "ROUNDTABLE_CIRCLE_BIN", bin_dir)?;
    let tower_bin = resolve_bin("conning-tower", "ROUNDTABLE_TOWER_BIN", bin_dir)?;

    // Stage 1: the-lunch lunch --date <date>
    let out = run_stage(&[lunch_bin.to_str().unwrap(), "lunch", "--date", &date], dry_run)
        .context("stage 'lunch' failed")?;
    if !out.status.success() {
        bail!("stage 'lunch' failed (exit {})", out.status);
    }

    // Parse table.json
    let state_home = xdg_state_home();
    let table_path = state_home.join("the-lunch").join(&date).join("table.json");

    let table = if table_path.exists() {
        parse_table(&table_path)?
    } else if dry_run {
        // In dry-run mode, table may not exist yet — that's fine
        roundtable::Table { dishes: vec![] }
    } else {
        bail!("table.json not found at {}", table_path.display());
    };

    if table.dishes.is_empty() {
        println!("no artifacts on the table today");
        return Ok(());
    }

    // Ledger path
    let data_home = xdg_data_home();
    let ledger = data_home.join("roundtable").join(&date).join("ledger.jsonl");

    // Stage 2: vicious-circle record for each artifact
    for dish in &table.dishes {
        if !dry_run && dedup_check(&ledger, &dish.path, &date) {
            println!("skipping '{}' (already recorded)", dish.title);
            continue;
        }
        if !dry_run {
            if let Some(parent) = ledger.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating ledger dir {}", parent.display()))?;
            }
        }
        let artifact_str = dish.path.to_string_lossy().to_string();
        let ledger_str = ledger.to_string_lossy().to_string();
        let out = run_stage(
            &[
                circle_bin.to_str().unwrap(),
                "record",
                &artifact_str,
                "--ledger",
                &ledger_str,
            ],
            dry_run,
        )
        .with_context(|| format!("stage 'critique:{}' failed", dish.path.display()))?;
        if !out.status.success() {
            bail!("stage 'critique:{}' failed (exit {})", dish.path.display(), out.status);
        }
    }

    // Stage 3: conning-tower compose
    let ledger_str = ledger.to_string_lossy().to_string();
    let out = run_stage(
        &[
            tower_bin.to_str().unwrap(),
            "compose",
            "--ledger",
            &ledger_str,
            "--date",
            &date,
        ],
        dry_run,
    )
    .context("stage 'compose' failed")?;
    if !out.status.success() {
        bail!("stage 'compose' failed (exit {})", out.status);
    }

    // Stage 4: conning-tower syndicate
    let out = run_stage(
        &[
            tower_bin.to_str().unwrap(),
            "syndicate",
            "--ledger",
            &ledger_str,
            "--date",
            &date,
            "--to",
            "columns",
        ],
        dry_run,
    )
    .context("stage 'syndicate' failed")?;
    if !out.status.success() {
        bail!("stage 'syndicate' failed (exit {})", out.status);
    }

    println!(
        "{} artifact(s) critiqued, column composed and syndicated to columns",
        table.dishes.len()
    );
    Ok(())
}

fn resolve_bin(tool: &str, env_var: &str, bin_dir: Option<&Path>) -> Result<PathBuf> {
    if let Ok(val) = std::env::var(env_var) {
        return Ok(PathBuf::from(val));
    }
    resolve_tool(tool, bin_dir)
        .with_context(|| format!("missing required tool: {tool}"))
}

fn xdg_state_home() -> PathBuf {
    if let Ok(v) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(v)
    } else {
        dirs_home().join(".local").join("state")
    }
}

fn xdg_data_home() -> PathBuf {
    if let Ok(v) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(v)
    } else {
        dirs_home().join(".local").join("share")
    }
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

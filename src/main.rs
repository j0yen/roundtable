use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use roundtable::{
    build_digest, current_iso_week, dedup_check, find_columns_since, iso_week_from_date,
    last_issue_date, parse_table, read_bon_mot_for_date, read_column_headline, resolve_tool,
    run_game, run_stage, BonMotSummary, DigestJson, Game, Opponent,
};
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    sigpipe::reset();
    let cli = Cli::parse();
    match cli.command {
        Commands::Session(args) => run_session(args),
        Commands::Digest(args) => run_digest(args),
        Commands::Bind(args) => run_bind(args),
        Commands::Games(args) => run_games(args),
        Commands::Weekly(args) => run_weekly(args),
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
    /// Surface yesterday's crowned bon mot and column headline (offline, local only)
    Digest(DigestArgs),
    /// Bind columns into an issue via new-yorker
    Bind(BindArgs),
    /// Play a debate game against an AI opponent
    Games(GamesArgs),
    /// Build and display a weekly digest across all sources
    Weekly(WeeklyArgs),
}

#[derive(Parser)]
struct DigestArgs {
    /// Date in YYYY-MM-DD format (default: yesterday)
    #[arg(long)]
    date: Option<String>,

    /// Output format: text (default) or json
    #[arg(long, default_value = "text")]
    format: String,

    /// Path to the vicious-circle ledger JSONL (overrides default)
    #[arg(long)]
    ledger: Option<PathBuf>,

    /// Path to the conning-tower columns directory (overrides default)
    #[arg(long)]
    columns_dir: Option<PathBuf>,
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

    /// After the session chain, also run the games stage
    #[arg(long)]
    with_games: bool,
}

#[derive(Parser)]
struct BindArgs {
    /// Only bind columns since this date (YYYY-MM-DD); defaults to last issue date or 1970-01-01
    #[arg(long)]
    since: Option<String>,

    /// Print plan without executing
    #[arg(long)]
    dry_run: bool,

    /// Directory to search for tool binaries (overrides PATH)
    #[arg(long)]
    bin_dir: Option<PathBuf>,

    /// Directory containing JSONL ledger files (overrides default)
    #[arg(long)]
    columns_dir: Option<PathBuf>,
}

#[derive(Parser)]
struct GamesArgs {
    /// Path to the article file to debate
    article: PathBuf,

    /// Opponent type: wordsmith, pedant, or contrarian (default: contrarian)
    #[arg(long, default_value = "contrarian")]
    opponent: String,

    /// Number of rounds (default: 3)
    #[arg(long, default_value = "3")]
    rounds: u8,

    /// Print plan without executing
    #[arg(long)]
    dry_run: bool,
}

#[derive(Parser)]
struct WeeklyArgs {
    /// ISO week to digest (e.g. 2026-W25); defaults to current week
    #[arg(long)]
    week: Option<String>,

    /// Derive week from a specific date (YYYY-MM-DD)
    #[arg(long)]
    date: Option<String>,

    /// Output format: text (default), markdown, or json
    #[arg(long, default_value = "text")]
    format: String,
}

fn today_date() -> Result<String> {
    let output = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .context("getting today's date")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Return the ISO date for yesterday (YYYY-MM-DD).
fn yesterday_date() -> Result<String> {
    let output = std::process::Command::new("date")
        .args(["-d", "yesterday", "+%Y-%m-%d"])
        .output()
        .context("getting yesterday's date")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Default path for the vicious-circle ledger.
/// Mirrors vicious-circle's own default: `$XDG_DATA_HOME/vicious-circle/ledger.jsonl`.
fn default_vc_ledger() -> PathBuf {
    let xdg = std::env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| format!("{}/.local/share", dirs_home().display()));
    PathBuf::from(xdg).join("vicious-circle").join("ledger.jsonl")
}

/// Default columns directory for conning-tower.
fn default_columns_dir() -> PathBuf {
    dirs_home()
        .join("wintermute")
        .join("conning-tower")
        .join("columns")
}

fn run_digest(args: DigestArgs) -> Result<()> {
    let date = match args.date {
        Some(d) => d,
        None => yesterday_date()?,
    };
    let ledger = args.ledger.unwrap_or_else(default_vc_ledger);
    let cols_dir = args.columns_dir.unwrap_or_else(default_columns_dir);
    let column_path = cols_dir.join(format!("{date}.md"));

    let bon_mot = read_bon_mot_for_date(&ledger, &date);
    let headline = read_column_headline(&column_path);

    match args.format.as_str() {
        "json" => {
            let out = DigestJson {
                date: date.clone(),
                bon_mot: bon_mot.map(|(line, author)| BonMotSummary { line, author }),
                column_headline: headline,
                fallback: false,
            };
            println!("{}", serde_json::to_string(&out).context("serialising digest")?);
        }
        _ => {
            match bon_mot {
                Some((line, author)) => {
                    let col_part = match headline {
                        Some(h) => format!("; column: {h}"),
                        None => String::new(),
                    };
                    println!(
                        "roundtable \u{00b7} {date} bon mot: \"{line}\" \u{2014} {author}{col_part}"
                    );
                }
                None => {
                    println!(
                        "roundtable \u{00b7} no session run for {date} \u{2014} run: roundtable session"
                    );
                }
            }
        }
    }
    Ok(())
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

    // Optional games stage
    if args.with_games {
        if let Some(first_dish) = table.dishes.first() {
            let game = Game {
                article: first_dish.path.clone(),
                opponent: Opponent::Contrarian,
                rounds: 3,
            };
            let result = run_game(&game, dry_run);
            println!("{result}");
        }
    }

    Ok(())
}

fn run_bind(args: BindArgs) -> Result<()> {
    let data_home = xdg_data_home();
    let issues_dir = data_home.join("roundtable").join("issues");
    let ledger_dir = args
        .columns_dir
        .unwrap_or_else(|| data_home.join("roundtable").join("ledger"));
    let bin_dir = args.bin_dir.as_deref();

    // Determine since_date
    let since_date = match args.since {
        Some(s) => s,
        None => last_issue_date(&issues_dir).unwrap_or_else(|| "1970-01-01".to_string()),
    };

    // Find columns since that date
    let columns = find_columns_since(&ledger_dir, &since_date);

    if columns.is_empty() {
        println!("nothing to bind (no new columns since {since_date})");
        return Ok(());
    }

    // Idempotency check: if an issue already exists covering since_date
    if let Some(existing) = last_issue_date(&issues_dir) {
        if existing.as_str() >= since_date.as_str() {
            println!("issue already exists for period since {since_date}: {existing}");
            return Ok(());
        }
    }

    // Resolve new-yorker binary
    let newyorker_bin_opt = if let Ok(val) = std::env::var("ROUNDTABLE_NEWYORKER_BIN") {
        Some(PathBuf::from(val))
    } else {
        resolve_tool("new-yorker", bin_dir)
    };

    if args.dry_run {
        let ny_display = newyorker_bin_opt
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "new-yorker".to_string());
        println!("[dry-run] would run: {ny_display} issue");
        println!("[dry-run] would run: {ny_display} cover");
        println!("[dry-run] {} column(s) since {since_date}", columns.len());
    } else {
        let newyorker_bin = match newyorker_bin_opt {
            Some(p) => p,
            None => {
                println!("new-yorker not found, skipping");
                std::process::exit(1);
            }
        };
        let ny_str = newyorker_bin.to_str().unwrap();
        let out = run_stage(&[ny_str, "issue"], false).context("new-yorker issue failed")?;
        if !out.status.success() {
            bail!("new-yorker issue failed (exit {})", out.status);
        }
        let out = run_stage(&[ny_str, "cover"], false).context("new-yorker cover failed")?;
        if !out.status.success() {
            bail!("new-yorker cover failed (exit {})", out.status);
        }
        println!("bound {} column(s) since {since_date}", columns.len());
    }

    Ok(())
}

fn run_games(args: GamesArgs) -> Result<()> {
    // Validate article exists
    if !args.article.exists() {
        bail!("article file not found: {}", args.article.display());
    }

    let opponent = match args.opponent.to_lowercase().as_str() {
        "wordsmith" => Opponent::Wordsmith,
        "pedant" => Opponent::Pedant,
        "contrarian" => Opponent::Contrarian,
        other => bail!("unknown opponent: {other}; use wordsmith, pedant, or contrarian"),
    };

    let game = Game {
        article: args.article,
        opponent,
        rounds: args.rounds,
    };

    let result = run_game(&game, args.dry_run);
    println!("{result}");
    Ok(())
}

fn run_weekly(args: WeeklyArgs) -> Result<()> {
    // Determine the week
    let week = if let Some(w) = args.week {
        w
    } else if let Some(d) = args.date {
        iso_week_from_date(&d)
            .with_context(|| format!("could not compute ISO week for date {d}"))?
    } else {
        current_iso_week()
    };

    let data_home = xdg_data_home();
    let store_dir = data_home.join("roundtable");
    let digest = build_digest(&store_dir, &week);

    match args.format.as_str() {
        "markdown" => {
            println!("## Week {}", digest.week);
            if !digest.columns.is_empty() {
                println!("\n### Columns");
                for col in &digest.columns {
                    println!("- {col}");
                }
            }
            if !digest.bon_mots.is_empty() {
                println!("\n### Bon Mots");
                for bm in &digest.bon_mots {
                    println!("- {bm}");
                }
            }
            if !digest.games.is_empty() {
                println!("\n### Games");
                for g in &digest.games {
                    println!("- {g}");
                }
            }
            if let Some(issue) = &digest.issue_date {
                println!("\n### Issue");
                println!("- {issue}");
            }
        }
        "json" => {
            let obj = serde_json::json!({
                "week": digest.week,
                "columns": digest.columns,
                "bon_mots": digest.bon_mots,
                "games": digest.games,
                "issue_date": digest.issue_date,
            });
            println!("{}", serde_json::to_string(&obj).context("serialising weekly digest")?);
        }
        _ => {
            // text
            println!("Week: {}", digest.week);
            println!("Columns: {}", digest.columns.len());
            println!("Bon mots: {}", digest.bon_mots.len());
            println!("Games: {}", digest.games.len());
            if let Some(issue) = &digest.issue_date {
                println!("Issue: {issue}");
            } else {
                println!("Issue: none");
            }
        }
    }

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

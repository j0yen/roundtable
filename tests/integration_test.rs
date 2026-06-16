use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn roundtable_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_roundtable"))
}

fn write_stub(dir: &Path, name: &str, script: &str) {
    let path = dir.join(name);
    fs::write(&path, script).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
}

fn make_stubs(bin_dir: &Path, state_home: &Path) {
    // the-lunch stub: writes table.json with one artifact
    let the_lunch_script = format!(
        r#"#!/bin/sh
# args: lunch --date <date>
DATE="$3"
DIR="{state_home}/the-lunch/$DATE"
mkdir -p "$DIR"
cat > "$DIR/table.json" << 'EOF'
{{"dishes":[{{"path":"/tmp/test-artifact.txt","title":"Test Artifact"}}]}}
EOF
"#,
        state_home = state_home.display()
    );
    write_stub(bin_dir, "the-lunch", &the_lunch_script);

    // vicious-circle stub: appends to ledger
    write_stub(
        bin_dir,
        "vicious-circle",
        r#"#!/bin/sh
# args: record <artifact> --ledger <ledger>
ARTIFACT="$2"
LEDGER="$4"
mkdir -p "$(dirname "$LEDGER")"
printf '{"artifact":"%s","date":"%s","crowned":"stub bon mot"}\n' "$ARTIFACT" "$ROUNDTABLE_SESSION_DATE" >> "$LEDGER"
"#,
    );

    // conning-tower stub: just exits 0
    write_stub(
        bin_dir,
        "conning-tower",
        r#"#!/bin/sh
echo "conning-tower stub: $*"
"#,
    );
}

#[test]
fn test_dry_run_prints_plan_no_files() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    let state_home = tmp.path().join("state");
    let data_home = tmp.path().join("data");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&state_home).unwrap();

    make_stubs(&bin_dir, &state_home);

    // Pre-create table.json for dry-run to read
    let table_dir = state_home.join("the-lunch").join("2026-01-01");
    fs::create_dir_all(&table_dir).unwrap();
    fs::write(
        table_dir.join("table.json"),
        r#"{"dishes":[{"path":"/tmp/test-artifact.txt","title":"Test Artifact"}]}"#,
    )
    .unwrap();

    let output = Command::new(roundtable_bin())
        .args(["session", "--dry-run", "--date", "2026-01-01", "--bin-dir"])
        .arg(&bin_dir)
        .env("XDG_STATE_HOME", &state_home)
        .env("XDG_DATA_HOME", &data_home)
        .env("ROUNDTABLE_SESSION_DATE", "2026-01-01")
        // Clear the tool env vars so it uses --bin-dir
        .env_remove("ROUNDTABLE_LUNCH_BIN")
        .env_remove("ROUNDTABLE_CIRCLE_BIN")
        .env_remove("ROUNDTABLE_TOWER_BIN")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "dry-run should exit 0");
    assert!(stdout.contains("[dry-run]"), "should print [dry-run] prefix");

    // No ledger file should be created
    let ledger = data_home.join("roundtable").join("2026-01-01").join("ledger.jsonl");
    assert!(!ledger.exists(), "ledger should not be created in dry-run");
}

#[test]
fn test_full_chain_with_stubs() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    let state_home = tmp.path().join("state");
    let data_home = tmp.path().join("data");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&state_home).unwrap();
    fs::create_dir_all(&data_home).unwrap();

    make_stubs(&bin_dir, &state_home);

    let output = Command::new(roundtable_bin())
        .args(["session", "--date", "2026-01-01", "--bin-dir"])
        .arg(&bin_dir)
        .env("XDG_STATE_HOME", &state_home)
        .env("XDG_DATA_HOME", &data_home)
        .env("ROUNDTABLE_SESSION_DATE", "2026-01-01")
        .env_remove("ROUNDTABLE_LUNCH_BIN")
        .env_remove("ROUNDTABLE_CIRCLE_BIN")
        .env_remove("ROUNDTABLE_TOWER_BIN")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "full chain should exit 0: {stderr}");
    assert!(
        stdout.contains("artifact") && stdout.contains("critiqued"),
        "should print summary: {stdout}"
    );

    let ledger = data_home.join("roundtable").join("2026-01-01").join("ledger.jsonl");
    assert!(ledger.exists(), "ledger should be created");
}

#[test]
fn test_missing_binary_exits_nonzero() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    let state_home = tmp.path().join("state");
    let data_home = tmp.path().join("data");
    fs::create_dir_all(&bin_dir).unwrap();

    // Only create the-lunch and vicious-circle, not conning-tower
    make_stubs(&bin_dir, &state_home);
    fs::remove_file(bin_dir.join("conning-tower")).unwrap();

    let output = Command::new(roundtable_bin())
        .args(["session", "--date", "2026-01-01", "--bin-dir"])
        .arg(&bin_dir)
        .env("XDG_STATE_HOME", &state_home)
        .env("XDG_DATA_HOME", &data_home)
        // Restrict PATH to only bin_dir so system conning-tower is not found
        .env("PATH", &bin_dir)
        .env_remove("ROUNDTABLE_LUNCH_BIN")
        .env_remove("ROUNDTABLE_CIRCLE_BIN")
        .env_remove("ROUNDTABLE_TOWER_BIN")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(!output.status.success(), "should exit non-zero for missing tool");
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("conning-tower"),
        "should name the missing tool: {combined}"
    );
}

#[test]
fn test_stage_failure_names_stage() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    let state_home = tmp.path().join("state");
    let data_home = tmp.path().join("data");
    fs::create_dir_all(&bin_dir).unwrap();

    make_stubs(&bin_dir, &state_home);

    // Replace the-lunch with a failing stub
    write_stub(&bin_dir, "the-lunch", "#!/bin/sh\nexit 1\n");

    let output = Command::new(roundtable_bin())
        .args(["session", "--date", "2026-01-01", "--bin-dir"])
        .arg(&bin_dir)
        .env("XDG_STATE_HOME", &state_home)
        .env("XDG_DATA_HOME", &data_home)
        .env_remove("ROUNDTABLE_LUNCH_BIN")
        .env_remove("ROUNDTABLE_CIRCLE_BIN")
        .env_remove("ROUNDTABLE_TOWER_BIN")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");
    eprintln!("combined: {combined}");

    assert!(!output.status.success(), "should exit non-zero on stage failure");
    assert!(
        combined.contains("lunch"),
        "should name the failed stage: {combined}"
    );
}

#[test]
fn test_dedup_no_double_record() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    let state_home = tmp.path().join("state");
    let data_home = tmp.path().join("data");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&data_home).unwrap();

    make_stubs(&bin_dir, &state_home);

    let run = || {
        Command::new(roundtable_bin())
            .args(["session", "--date", "2026-01-01", "--bin-dir"])
            .arg(&bin_dir)
            .env("XDG_STATE_HOME", &state_home)
            .env("XDG_DATA_HOME", &data_home)
            .env("ROUNDTABLE_SESSION_DATE", "2026-01-01")
            .env_remove("ROUNDTABLE_LUNCH_BIN")
            .env_remove("ROUNDTABLE_CIRCLE_BIN")
            .env_remove("ROUNDTABLE_TOWER_BIN")
            .output()
            .unwrap()
    };

    let out1 = run();
    assert!(out1.status.success(), "first run should succeed: {}", String::from_utf8_lossy(&out1.stderr));

    let out2 = run();
    assert!(out2.status.success(), "second run should succeed: {}", String::from_utf8_lossy(&out2.stderr));

    let ledger = data_home.join("roundtable").join("2026-01-01").join("ledger.jsonl");
    let content = fs::read_to_string(&ledger).unwrap();
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 1, "ledger should have exactly 1 line after 2 runs, got: {content}");
}

#[test]
fn test_empty_table_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    let state_home = tmp.path().join("state");
    let data_home = tmp.path().join("data");
    fs::create_dir_all(&bin_dir).unwrap();

    // the-lunch stub that writes empty table
    let the_lunch_script = format!(
        r#"#!/bin/sh
DATE="$3"
DIR="{state_home}/the-lunch/$DATE"
mkdir -p "$DIR"
echo '{{"dishes":[]}}' > "$DIR/table.json"
"#,
        state_home = state_home.display()
    );
    write_stub(&bin_dir, "the-lunch", &the_lunch_script);
    write_stub(&bin_dir, "vicious-circle", "#!/bin/sh\nexit 0\n");
    write_stub(&bin_dir, "conning-tower", "#!/bin/sh\nexit 0\n");

    let output = Command::new(roundtable_bin())
        .args(["session", "--date", "2026-01-01", "--bin-dir"])
        .arg(&bin_dir)
        .env("XDG_STATE_HOME", &state_home)
        .env("XDG_DATA_HOME", &data_home)
        .env_remove("ROUNDTABLE_LUNCH_BIN")
        .env_remove("ROUNDTABLE_CIRCLE_BIN")
        .env_remove("ROUNDTABLE_TOWER_BIN")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "empty table should exit 0: {stderr}");
    assert!(
        stdout.contains("no artifacts on the table today"),
        "should report empty table: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Digest tests (AC2, AC3, AC4)
// ---------------------------------------------------------------------------

/// Write a minimal vicious-circle ledger JSONL for the given date with a crowned bon mot.
fn write_vc_ledger(path: &std::path::Path, date: &str, line: &str, author: &str) {
    let json = format!(
        "{{\"date\":\"{date}\",\"artifact\":\"test.txt\",\"verdicts\":[],\"crosses\":[],\"bon_mot\":{{\"line\":\"{line}\",\"author\":\"{author}\",\"target\":\"test.txt\",\"score\":8.5,\"peer_reaction\":0.6,\"rank\":8.65}}}}\n"
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, json).unwrap();
}

/// Write a minimal conning-tower column markdown file.
fn write_column_md(path: &std::path::Path, headline: &str) {
    let md = format!("# {headline}\n\nBody of the column.\n");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, md).unwrap();
}

#[test]
fn test_digest_with_fixture_ledger_and_column_text() {
    let tmp = TempDir::new().unwrap();
    let data_home = tmp.path().join("data");
    let cols_dir = tmp.path().join("columns");

    let date = "2026-01-15";
    let ledger_path = data_home.join("vicious-circle").join("ledger.jsonl");
    write_vc_ledger(&ledger_path, date, "A masterpiece of mediocrity.", "parker");

    let col_path = cols_dir.join(format!("{date}.md"));
    let headline = "2026-01-15 — The Conning Tower";
    write_column_md(&col_path, headline);

    let output = Command::new(roundtable_bin())
        .args([
            "digest",
            "--date", date,
            "--ledger", ledger_path.to_str().unwrap(),
            "--columns-dir", cols_dir.to_str().unwrap(),
            "--format", "text",
        ])
        .env("XDG_DATA_HOME", &data_home)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "digest should exit 0: {stderr}");
    assert!(stdout.contains("A masterpiece of mediocrity."), "should include bon mot line: {stdout}");
    assert!(stdout.contains("parker"), "should include author: {stdout}");
    assert!(stdout.contains("The Conning Tower"), "should include column headline: {stdout}");
}

#[test]
fn test_digest_with_fixture_ledger_json_format() {
    let tmp = TempDir::new().unwrap();
    let data_home = tmp.path().join("data");
    let cols_dir = tmp.path().join("columns");

    let date = "2026-01-15";
    let ledger_path = data_home.join("vicious-circle").join("ledger.jsonl");
    write_vc_ledger(&ledger_path, date, "It has all the spontaneity of a traffic jam.", "benchley");

    let col_path = cols_dir.join(format!("{date}.md"));
    write_column_md(&col_path, "2026-01-15 — The Conning Tower");

    let output = Command::new(roundtable_bin())
        .args([
            "digest",
            "--date", date,
            "--ledger", ledger_path.to_str().unwrap(),
            "--columns-dir", cols_dir.to_str().unwrap(),
            "--format", "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "digest --format json should exit 0: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("digest --format json should emit valid JSON");
    assert_eq!(parsed["date"].as_str().unwrap(), date);
    assert_eq!(
        parsed["bon_mot"]["line"].as_str().unwrap(),
        "It has all the spontaneity of a traffic jam."
    );
    assert_eq!(parsed["bon_mot"]["author"].as_str().unwrap(), "benchley");
    assert!(parsed["column_headline"].as_str().is_some(), "should have column headline");
}

#[test]
fn test_digest_fallback_no_session() {
    // AC3: no session run for date → prints fallback, exits 0
    let tmp = TempDir::new().unwrap();
    let data_home = tmp.path().join("data");
    let cols_dir = tmp.path().join("columns");
    // Ledger file does not exist at all
    let ledger_path = data_home.join("vicious-circle").join("ledger.jsonl");

    let output = Command::new(roundtable_bin())
        .args([
            "digest",
            "--date", "2026-01-01",
            "--ledger", ledger_path.to_str().unwrap(),
            "--columns-dir", cols_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "digest fallback should exit 0: {stderr}");
    assert!(
        stdout.contains("no session run"),
        "should print fallback: {stdout}"
    );
}

#[test]
fn test_sessionstart_hook_exit_zero_empty_xdg() {
    // AC4: hook must exit 0 and emit exactly one line even with no ledger/columns
    let tmp = TempDir::new().unwrap();
    let hook = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("hooks")
        .join("roundtable-sessionstart.sh");

    // Point to a non-existent roundtable binary so the hook takes the fallback path
    let fake_bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&fake_bin_dir).unwrap();

    // Use absolute path to bash; PATH only needs to exclude 'roundtable'
    let bash = std::process::Command::new("which")
        .arg("bash")
        .output()
        .unwrap();
    let bash_path = String::from_utf8_lossy(&bash.stdout).trim().to_string();

    let output = Command::new(&bash_path)
        .arg(&hook)
        .env("XDG_DATA_HOME", tmp.path().join("data"))
        .env("HOME", tmp.path())
        .env("PATH", &fake_bin_dir) // empty bin dir — no 'roundtable' found
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "hook must exit 0: {stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "hook should emit exactly one line: {stdout:?}");
}

// ---------------------------------------------------------------------------
// PRD 1: bind tests
// ---------------------------------------------------------------------------

#[test]
fn test_bind_dry_run_no_files() {
    let tmp = TempDir::new().unwrap();
    let data_home = tmp.path().join("data");
    let ledger_dir = data_home.join("roundtable").join("ledger");
    fs::create_dir_all(&ledger_dir).unwrap();

    // Write a ledger entry with a recent date
    fs::write(
        ledger_dir.join("columns.jsonl"),
        "{\"date\":\"2026-06-10\",\"path\":\"/tmp/col1.md\",\"title\":\"Column One\"}\n",
    )
    .unwrap();

    let output = Command::new(roundtable_bin())
        .args(["bind", "--dry-run", "--since", "2026-01-01", "--columns-dir"])
        .arg(&ledger_dir)
        .env("XDG_DATA_HOME", &data_home)
        .env_remove("ROUNDTABLE_NEWYORKER_BIN")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    // dry-run: no issue files should be created
    let issues_dir = data_home.join("roundtable").join("issues");
    let no_issues = !issues_dir.exists()
        || issues_dir.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true);
    assert!(no_issues, "no issue files should be created in dry-run");
    // Should mention dry-run in output
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("[dry-run]") || combined.contains("nothing to bind") || output.status.success(),
        "should either print dry-run plan or exit cleanly: {combined}"
    );
}

#[test]
fn test_bind_no_columns() {
    let tmp = TempDir::new().unwrap();
    let data_home = tmp.path().join("data");
    let ledger_dir = data_home.join("roundtable").join("ledger");
    fs::create_dir_all(&ledger_dir).unwrap();

    // Write a ledger entry with an OLD date (before since_date)
    fs::write(
        ledger_dir.join("columns.jsonl"),
        "{\"date\":\"1969-12-31\",\"path\":\"/tmp/old.md\",\"title\":\"Old Column\"}\n",
    )
    .unwrap();

    let output = Command::new(roundtable_bin())
        .args(["bind", "--since", "2026-01-01", "--columns-dir"])
        .arg(&ledger_dir)
        .env("XDG_DATA_HOME", &data_home)
        .env_remove("ROUNDTABLE_NEWYORKER_BIN")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "no-columns case should exit 0: {stderr}");
    assert!(
        stdout.contains("nothing to bind"),
        "should print 'nothing to bind': {stdout}"
    );
}

#[test]
fn test_bind_with_stub_newyorker() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    let data_home = tmp.path().join("data");
    let ledger_dir = data_home.join("roundtable").join("ledger");
    let issues_dir = data_home.join("roundtable").join("issues");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&ledger_dir).unwrap();
    fs::create_dir_all(&issues_dir).unwrap();

    // Write a ledger entry with a recent date
    fs::write(
        ledger_dir.join("columns.jsonl"),
        "{\"date\":\"2026-06-10\",\"path\":\"/tmp/col1.md\",\"title\":\"Column One\"}\n",
    )
    .unwrap();

    // Stub new-yorker that exits 0
    write_stub(
        &bin_dir,
        "new-yorker",
        "#!/bin/sh\necho \"new-yorker stub: $*\"\n",
    );

    let output = Command::new(roundtable_bin())
        .args(["bind", "--since", "2026-01-01", "--columns-dir"])
        .arg(&ledger_dir)
        .env("XDG_DATA_HOME", &data_home)
        .env("ROUNDTABLE_NEWYORKER_BIN", bin_dir.join("new-yorker"))
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "bind with stub new-yorker should exit 0: {stderr}");
    assert!(stdout.contains("bound"), "should print 'bound': {stdout}");
}

// ---------------------------------------------------------------------------
// PRD 2: games tests
// ---------------------------------------------------------------------------

#[test]
fn test_games_default_opponent() {
    let tmp = TempDir::new().unwrap();
    let article = tmp.path().join("article.md");
    fs::write(&article, "# Test Article\nSome content.\n").unwrap();

    let output = Command::new(roundtable_bin())
        .args(["games"])
        .arg(&article)
        .env("PATH", "/nonexistent") // ensure thanatopsis not found
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    // Default opponent is Contrarian — stub transcript should have round headers
    assert!(output.status.success(), "games should exit 0: {stderr}");
    assert!(
        stdout.contains("## Round 1"),
        "stub transcript should have round headers: {stdout}"
    );
}

#[test]
fn test_games_dry_run() {
    let tmp = TempDir::new().unwrap();
    let article = tmp.path().join("article.md");
    fs::write(&article, "# Test Article\nSome content.\n").unwrap();

    let output = Command::new(roundtable_bin())
        .args(["games", "--dry-run"])
        .arg(&article)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "dry-run games should exit 0: {stderr}");
    assert!(
        stdout.contains("would start game"),
        "dry-run should print 'would start game': {stdout}"
    );
    assert!(
        stdout.contains("Contrarian"),
        "dry-run should mention Contrarian: {stdout}"
    );
    assert!(
        stdout.contains("3 rounds"),
        "dry-run should mention 3 rounds: {stdout}"
    );
}

#[test]
fn test_games_missing_article() {
    let output = Command::new(roundtable_bin())
        .args(["games", "/nonexistent/path/article.md"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");
    eprintln!("combined: {combined}");

    assert!(!output.status.success(), "missing article should exit non-zero");
    assert!(
        combined.contains("article"),
        "error should mention 'article': {combined}"
    );
}

#[test]
fn test_games_stub_transcript() {
    let tmp = TempDir::new().unwrap();
    let article = tmp.path().join("article.md");
    fs::write(&article, "# Test Article\nSome content.\n").unwrap();

    let output = Command::new(roundtable_bin())
        .args(["games", "--rounds", "3"])
        .arg(&article)
        .env("PATH", "/nonexistent") // ensure thanatopsis not found → stub
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "stub transcript should exit 0: {stderr}");
    assert!(stdout.contains("## Round 1"), "should have Round 1: {stdout}");
    assert!(stdout.contains("## Round 2"), "should have Round 2: {stdout}");
    assert!(stdout.contains("## Round 3"), "should have Round 3: {stdout}");
}

#[test]
fn test_session_without_games_flag_skips_games() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    let state_home = tmp.path().join("state");
    let data_home = tmp.path().join("data");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&state_home).unwrap();
    fs::create_dir_all(&data_home).unwrap();

    make_stubs(&bin_dir, &state_home);

    let output = Command::new(roundtable_bin())
        // No --with-games flag
        .args(["session", "--date", "2026-01-01", "--bin-dir"])
        .arg(&bin_dir)
        .env("XDG_STATE_HOME", &state_home)
        .env("XDG_DATA_HOME", &data_home)
        .env("ROUNDTABLE_SESSION_DATE", "2026-01-01")
        .env_remove("ROUNDTABLE_LUNCH_BIN")
        .env_remove("ROUNDTABLE_CIRCLE_BIN")
        .env_remove("ROUNDTABLE_TOWER_BIN")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "session without --with-games should exit 0: {stderr}");
    // Should NOT contain game-related output
    assert!(
        !stdout.contains("would start game") && !stdout.contains("## Round"),
        "session without --with-games should not run games: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// PRD 3: weekly digest tests
// ---------------------------------------------------------------------------

#[test]
fn test_digest_empty_store() {
    let tmp = TempDir::new().unwrap();
    let data_home = tmp.path().join("data");
    // Don't create the store dir at all

    let output = Command::new(roundtable_bin())
        .args(["weekly", "--week", "2026-W25"])
        .env("XDG_DATA_HOME", &data_home)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "empty store should exit 0: {stderr}");
    assert!(
        stdout.contains("Columns: 0"),
        "empty store should show 0 columns: {stdout}"
    );
}

#[test]
fn test_digest_default_week() {
    let tmp = TempDir::new().unwrap();
    let data_home = tmp.path().join("data");
    fs::create_dir_all(data_home.join("roundtable")).unwrap();

    let output = Command::new(roundtable_bin())
        // No --week flag: should use current week
        .args(["weekly"])
        .env("XDG_DATA_HOME", &data_home)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "default week should exit 0: {stderr}");
    assert!(stdout.contains("Week:"), "should contain Week: line: {stdout}");
}

#[test]
fn test_digest_markdown_format() {
    let tmp = TempDir::new().unwrap();
    let data_home = tmp.path().join("data");
    fs::create_dir_all(data_home.join("roundtable")).unwrap();

    let output = Command::new(roundtable_bin())
        .args(["weekly", "--week", "2026-W25", "--format", "markdown"])
        .env("XDG_DATA_HOME", &data_home)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "markdown format should exit 0: {stderr}");
    assert!(
        stdout.contains("## Week"),
        "markdown format should have '## Week' header: {stdout}"
    );
}

#[test]
fn test_digest_all_sources() {
    let tmp = TempDir::new().unwrap();
    let data_home = tmp.path().join("data");
    let store_dir = data_home.join("roundtable");
    let ledger_dir = store_dir.join("ledger");
    let games_dir = store_dir.join("games");
    fs::create_dir_all(&ledger_dir).unwrap();
    fs::create_dir_all(&games_dir).unwrap();

    // Write a ledger entry in 2026-W25 (2026-06-15 is a Monday in that week)
    fs::write(
        ledger_dir.join("test.jsonl"),
        "{\"date\":\"2026-06-15\",\"path\":\"/tmp/col.md\",\"title\":\"Test Column\",\"crowned\":\"A witty remark\"}\n",
    )
    .unwrap();

    // Write a game transcript file for this week
    fs::write(
        games_dir.join("2026-W25-game.txt"),
        "## Round 1\n[game content]\n",
    )
    .unwrap();

    let output = Command::new(roundtable_bin())
        .args(["weekly", "--week", "2026-W25", "--format", "markdown"])
        .env("XDG_DATA_HOME", &data_home)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "all-sources digest should exit 0: {stderr}");
    assert!(stdout.contains("## Week"), "should have week header: {stdout}");
    assert!(stdout.contains("Test Column"), "should include column: {stdout}");
    assert!(stdout.contains("A witty remark"), "should include bon mot: {stdout}");
    assert!(stdout.contains("2026-W25-game.txt"), "should include game: {stdout}");
}

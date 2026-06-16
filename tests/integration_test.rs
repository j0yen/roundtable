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

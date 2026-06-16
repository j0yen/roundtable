//! AC6: Re-running `session` for the same date does not double-record an
//! artifact already in the ledger for that date.

use std::io::Write as _;
use std::process::Command;

fn roundtable_bin() -> std::path::PathBuf {
    if let Some(p) = option_env!("CARGO_BIN_EXE_roundtable") {
        return std::path::PathBuf::from(p);
    }
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push("debug");
    p.push("roundtable");
    p
}

fn count_ledger_lines(path: &std::path::Path) -> usize {
    if !path.exists() {
        return 0;
    }
    let contents = std::fs::read_to_string(path).expect("read ledger");
    contents
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

#[test]
fn ac6_no_double_record_on_rerun() {
    let bin = roundtable_bin();
    if !bin.exists() {
        eprintln!("AC6: binary not found, skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tmp");
    let date = "2026-06-15";

    // Pre-create table.json with one artifact.
    let state_dir = tmp.path().join("the-lunch").join(date);
    std::fs::create_dir_all(&state_dir).expect("mkdir");
    let artifact = tmp.path().join("haiku.txt");
    std::fs::write(&artifact, "content").expect("write");

    let table_json = format!(
        r#"{{"date":"{date}","created":"{date}T00:00:00+00:00","dishes":[
            {{"present":true,"provenance":{{"artifact_path":"{}"}},"kind":"Haiku","source":"haiku","title":null,"content":"x"}}
        ]}}"#,
        artifact.display()
    );
    let table_path = state_dir.join("table.json");
    std::fs::write(&table_path, &table_json).expect("write table");

    let ledger = tmp.path().join("ledger.jsonl");
    let columns_dir = tmp.path().join("columns");

    // Pre-write the ledger entry so the artifact is "already recorded".
    let entry = format!(
        r#"{{"date":"{date}","artifact_path":"{}","crowned":"bon mot","round_id":"r1"}}"#,
        artifact.display()
    );
    {
        let mut f = std::fs::File::create(&ledger).expect("create ledger");
        writeln!(f, "{entry}").expect("write");
    }

    // All stubs exit 0; vicious-circle should NOT be called for the already-recorded artifact.
    // To verify, we use a stub that would append to ledger if called, and check length stays 1.
    // We use the stub that exits 0 and appends a dummy line — if roundtable deduplicates,
    // the stub won't be called for this artifact and the ledger stays at 1 line.
    let vc_script = format!(
        "#!/bin/sh\necho '{{\"date\":\"{date}\",\"artifact_path\":\"/should/not/appear\",\"crowned\":\"extra\",\"round_id\":\"r2\"}}' >> {}\nexit 0\n",
        ledger.display()
    );
    for name in &["the-lunch", "conning-tower"] {
        let p = tmp.path().join(name);
        std::fs::write(&p, "#!/bin/sh\nexit 0\n").expect("write stub");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let vc_p = tmp.path().join("vicious-circle");
    std::fs::write(&vc_p, &vc_script).expect("write vc stub");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&vc_p, std::fs::Permissions::from_mode(0o755)).expect("chmod vc");

    let before_lines = count_ledger_lines(&ledger);
    assert_eq!(before_lines, 1, "ledger should start with 1 entry");

    let output = Command::new(&bin)
        .args([
            "session",
            "--date",
            date,
            "--ledger",
            ledger.to_str().expect("utf8"),
            "--columns-dir",
            columns_dir.to_str().expect("utf8"),
            "--bin-dir",
            tmp.path().to_str().expect("utf8"),
        ])
        .env("XDG_STATE_HOME", tmp.path())
        .output()
        .expect("run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "AC6: session should succeed; stdout={stdout}; stderr={stderr}"
    );

    let after_lines = count_ledger_lines(&ledger);
    assert_eq!(
        after_lines, before_lines,
        "AC6: ledger should not gain entries on re-run; before={before_lines}, after={after_lines}"
    );
}

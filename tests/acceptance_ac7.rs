//! AC7: `roundtable session` over an empty table (no dishes) completes cleanly,
//! reports "no artifacts on the table today," and exits 0.

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

#[test]
fn ac7_empty_table_exits_zero_reports_empty() {
    let bin = roundtable_bin();
    if !bin.exists() {
        eprintln!("AC7: binary not found, skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tmp");
    let date = "2026-06-15";

    // Create table.json with no present dishes.
    let state_dir = tmp.path().join("the-lunch").join(date);
    std::fs::create_dir_all(&state_dir).expect("mkdir");
    let table_json = format!(
        r#"{{"date":"{date}","created":"{date}T00:00:00+00:00","dishes":[
            {{"present":false,"provenance":{{"artifact_path":null}},"kind":"Haiku","source":"haiku","title":null,"content":""}}
        ]}}"#
    );
    std::fs::write(state_dir.join("table.json"), &table_json).expect("write table");

    let ledger = tmp.path().join("ledger.jsonl");
    let columns_dir = tmp.path().join("columns");

    // Stub binaries — the-lunch succeeds.
    for name in &["the-lunch", "vicious-circle", "conning-tower"] {
        let p = tmp.path().join(name);
        std::fs::write(&p, "#!/bin/sh\nexit 0\n").expect("write stub");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

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
        "AC7: empty table → exit 0; stdout={stdout}; stderr={stderr}"
    );

    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("no artifacts"),
        "AC7: output should mention 'no artifacts'; got: {combined}"
    );
}

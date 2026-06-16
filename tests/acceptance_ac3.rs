//! AC3: With stub binaries on `--bin-dir`, `roundtable session` runs the full
//! chain and prints a summary naming the crowned line and column destination.

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

#[test]
fn ac3_stub_bins_full_chain_summary() {
    let bin = roundtable_bin();
    if !bin.exists() {
        eprintln!("AC3: binary not found, skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tmp");
    let date = "2026-06-15";

    // Create the state directory that the-lunch would produce.
    let state_dir = tmp.path().join("the-lunch").join(date);
    std::fs::create_dir_all(&state_dir).expect("create state dir");

    // Write a table.json with one present artifact.
    let artifact_path = tmp.path().join("haiku.txt");
    std::fs::write(&artifact_path, "test haiku content").expect("write artifact");

    let table_json = format!(
        r#"{{
            "date": "{date}",
            "created": "{date}T00:00:00+00:00",
            "dishes": [
                {{
                    "present": true,
                    "provenance": {{"artifact_path": "{}"}},
                    "kind": "Haiku",
                    "source": "haiku",
                    "title": null,
                    "content": "test haiku"
                }}
            ]
        }}"#,
        artifact_path.display()
    );
    std::fs::write(state_dir.join("table.json"), &table_json).expect("write table.json");

    // Pre-write a ledger entry so vicious-circle record "already recorded" path triggers.
    let ledger_path = tmp.path().join("ledger.jsonl");
    let ledger_entry = format!(
        r#"{{"date":"{date}","artifact_path":"{}","crowned":"the test crowned bon mot","round_id":"r1"}}"#,
        artifact_path.display()
    );
    let mut ledger_file = std::fs::File::create(&ledger_path).expect("create ledger");
    writeln!(ledger_file, "{ledger_entry}").expect("write ledger");

    let columns_dir = tmp.path().join("columns");

    // Create stubs for vicious-circle and conning-tower (exit 0).
    // the-lunch doesn't need to write the state dir in stub mode since we pre-created it.
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
            ledger_path.to_str().expect("utf8"),
            "--columns-dir",
            columns_dir.to_str().expect("utf8"),
            "--bin-dir",
            tmp.path().to_str().expect("utf8"),
        ])
        // Set XDG_STATE_HOME so roundtable resolves table.json from tmp dir.
        .env("XDG_STATE_HOME", tmp.path())
        .output()
        .expect("run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "AC3: session should succeed with stubs; stdout={stdout}; stderr={stderr}"
    );

    // Summary must mention the crowned bon mot and the column destination.
    assert!(
        stdout.contains("the test crowned bon mot"),
        "AC3: summary should name the crowned bon mot; stdout={stdout}"
    );
    assert!(
        stdout.contains(columns_dir.to_str().expect("utf8")),
        "AC3: summary should name the column destination; stdout={stdout}"
    );
}

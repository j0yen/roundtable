//! AC5: A stage failure (stub exits non-zero) makes `session` exit non-zero
//! and name the failed stage (`lunch`/`critique:<artifact>`/`compose`/`syndicate`).

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
fn ac5_lunch_failure_names_lunch_stage() {
    let bin = roundtable_bin();
    if !bin.exists() {
        eprintln!("AC5: binary not found, skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tmp");

    // the-lunch exits 1 (failure).
    let p = tmp.path().join("the-lunch");
    std::fs::write(&p, "#!/bin/sh\nexit 1\n").expect("write stub");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    // Other tools exist but shouldn't be reached.
    for name in &["vicious-circle", "conning-tower"] {
        let stub = tmp.path().join(name);
        std::fs::write(&stub, "#!/bin/sh\nexit 0\n").expect("write stub");
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    let ledger = tmp.path().join("ledger.jsonl");
    let output = Command::new(&bin)
        .args([
            "session",
            "--date",
            "2026-06-15",
            "--ledger",
            ledger.to_str().expect("utf8"),
            "--bin-dir",
            tmp.path().to_str().expect("utf8"),
        ])
        .env("XDG_STATE_HOME", tmp.path())
        .output()
        .expect("run");

    assert!(!output.status.success(), "AC5: lunch failure → non-zero exit");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        combined.contains("lunch"),
        "AC5: error should name stage `lunch`; got: {combined}"
    );
}

#[test]
fn ac5_critique_failure_names_critique_stage() {
    let bin = roundtable_bin();
    if !bin.exists() {
        eprintln!("AC5: binary not found, skipping");
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
    std::fs::write(state_dir.join("table.json"), &table_json).expect("write table");

    let ledger = tmp.path().join("ledger.jsonl");

    // the-lunch succeeds; vicious-circle fails.
    let lunch_stub = tmp.path().join("the-lunch");
    std::fs::write(&lunch_stub, "#!/bin/sh\nexit 0\n").expect("write");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&lunch_stub, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let vc_stub = tmp.path().join("vicious-circle");
    std::fs::write(&vc_stub, "#!/bin/sh\nexit 2\n").expect("write");
    std::fs::set_permissions(&vc_stub, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let ct_stub = tmp.path().join("conning-tower");
    std::fs::write(&ct_stub, "#!/bin/sh\nexit 0\n").expect("write");
    std::fs::set_permissions(&ct_stub, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let output = Command::new(&bin)
        .args([
            "session",
            "--date",
            date,
            "--ledger",
            ledger.to_str().expect("utf8"),
            "--bin-dir",
            tmp.path().to_str().expect("utf8"),
        ])
        .env("XDG_STATE_HOME", tmp.path())
        .output()
        .expect("run");

    assert!(!output.status.success(), "AC5: critique failure → non-zero");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        combined.contains("critique"),
        "AC5: error should name stage `critique:…`; got: {combined}"
    );
}

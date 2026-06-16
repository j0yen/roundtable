//! AC4: A missing required binary makes `roundtable session` exit non-zero
//! with a message naming the missing tool.

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
fn ac4_missing_binary_exits_nonzero_names_tool() {
    let bin = roundtable_bin();
    if !bin.exists() {
        eprintln!("AC4: binary not found, skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tmp");
    // --bin-dir points to empty dir → all tools missing.
    let output = Command::new(&bin)
        .args([
            "session",
            "--date",
            "2026-06-15",
            "--bin-dir",
            tmp.path().to_str().expect("utf8"),
        ])
        .output()
        .expect("run");

    assert!(
        !output.status.success(),
        "AC4: must exit non-zero when a required binary is missing"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");

    // The error message must name the missing tool.
    let names_a_tool = combined.contains("the-lunch")
        || combined.contains("vicious-circle")
        || combined.contains("conning-tower");
    assert!(
        names_a_tool,
        "AC4: error output should name a missing required binary; got: {combined}"
    );
}

#[test]
fn ac4_missing_conning_tower_names_it() {
    let bin = roundtable_bin();
    if !bin.exists() {
        eprintln!("AC4: binary not found, skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tmp");
    // Provide the-lunch and vicious-circle but NOT conning-tower.
    for name in &["the-lunch", "vicious-circle"] {
        let p = tmp.path().join(name);
        std::fs::write(&p, "#!/bin/sh\nexit 0\n").expect("write stub");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    let output = Command::new(&bin)
        .args([
            "session",
            "--date",
            "2026-06-15",
            "--bin-dir",
            tmp.path().to_str().expect("utf8"),
        ])
        .output()
        .expect("run");

    assert!(!output.status.success(), "AC4: missing conning-tower → non-zero");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        combined.contains("conning-tower"),
        "AC4: error should name `conning-tower`; got: {combined}"
    );
}

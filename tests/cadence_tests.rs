use roundtable::cadence::{weekday_of, Cadence, Weekday};
use std::process::Command;

fn roundtable_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_roundtable"))
}

#[test]
fn test_default_cadence_has_five_session_days() {
    let c = Cadence::default();
    assert_eq!(c.session_days.len(), 5);
}

#[test]
fn test_is_session_day_monday() {
    let c = Cadence::default();
    // 2026-06-15 is a Monday
    assert!(c.is_session_day("2026-06-15"), "Monday should be a session day");
}

#[test]
fn test_is_session_day_saturday_false() {
    let c = Cadence::default();
    // 2026-06-13 is a Saturday
    assert!(!c.is_session_day("2026-06-13"), "Saturday should not be a session day");
}

#[test]
fn test_is_bind_day_sunday() {
    let c = Cadence::default();
    // 2026-06-14 is a Sunday
    assert!(c.is_bind_day("2026-06-14"), "Sunday should be the bind day");
}

#[test]
fn test_weekday_of_monday() {
    // 2026-06-15 is a Monday
    assert_eq!(weekday_of("2026-06-15").unwrap(), Weekday::Mon);
}

#[test]
fn test_cadence_save_reload_roundtrip() {
    let tmp = std::env::temp_dir().join(format!("rt-cadence-test-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let path = tmp.join("cadence.json");

    let c = Cadence::default();
    c.save(&path).unwrap();

    let loaded = Cadence::from_file(&path).unwrap();
    assert_eq!(loaded.session_days.len(), c.session_days.len());
    assert_eq!(loaded.bind_day, c.bind_day);
    assert_eq!(loaded.digest_day, c.digest_day);
    assert_eq!(loaded.games_enabled, c.games_enabled);

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn test_cadence_show_text() {
    let tmp = std::env::temp_dir().join(format!("rt-cadence-show-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();

    let output = Command::new(roundtable_bin())
        .args(["cadence", "show"])
        .env("XDG_DATA_HOME", &tmp)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "cadence show should exit 0: {stderr}");
    assert!(stdout.contains("session days"), "should show session days: {stdout}");
    assert!(stdout.contains("bind day"), "should show bind day: {stdout}");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn test_cadence_show_json() {
    let tmp = std::env::temp_dir().join(format!("rt-cadence-json-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();

    let output = Command::new(roundtable_bin())
        .args(["cadence", "show", "--format", "json"])
        .env("XDG_DATA_HOME", &tmp)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "cadence show --format json should exit 0: {stderr}");
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("cadence show --format json should emit valid JSON");
    assert!(v.get("session_days").is_some(), "JSON should have session_days");
    assert!(v.get("bind_day").is_some(), "JSON should have bind_day");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn test_cadence_next_from_saturday() {
    let tmp = std::env::temp_dir().join(format!("rt-cadence-next-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();

    // 2026-06-13 is Saturday
    // next session = Mon 2026-06-15
    // next bind (Sun) = 2026-06-14
    // next digest (Mon) = 2026-06-15
    let output = Command::new(roundtable_bin())
        .args(["cadence", "next", "--from", "2026-06-13"])
        .env("XDG_DATA_HOME", &tmp)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("stdout: {stdout}\nstderr: {stderr}");

    assert!(output.status.success(), "cadence next should exit 0: {stderr}");
    assert!(stdout.contains("next session"), "should show next session: {stdout}");
    assert!(stdout.contains("next bind"), "should show next bind: {stdout}");
    assert!(stdout.contains("next digest"), "should show next digest: {stdout}");

    std::fs::remove_dir_all(&tmp).ok();
}

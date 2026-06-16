//! Publication schedule management for roundtable.
//!
//! Tracks which days to run `session`, `bind`, and `digest`, and provides
//! helpers to compute next-occurrence dates without any external date crates.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ---------------------------------------------------------------------------
// Weekday
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    /// 0-based index where Mon=0, Sun=6.
    pub fn index(self) -> u32 {
        match self {
            Weekday::Mon => 0,
            Weekday::Tue => 1,
            Weekday::Wed => 2,
            Weekday::Thu => 3,
            Weekday::Fri => 4,
            Weekday::Sat => 5,
            Weekday::Sun => 6,
        }
    }

    pub fn from_index(i: u32) -> Self {
        match i % 7 {
            0 => Weekday::Mon,
            1 => Weekday::Tue,
            2 => Weekday::Wed,
            3 => Weekday::Thu,
            4 => Weekday::Fri,
            5 => Weekday::Sat,
            _ => Weekday::Sun,
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "mon" | "monday" => Ok(Weekday::Mon),
            "tue" | "tuesday" => Ok(Weekday::Tue),
            "wed" | "wednesday" => Ok(Weekday::Wed),
            "thu" | "thursday" => Ok(Weekday::Thu),
            "fri" | "friday" => Ok(Weekday::Fri),
            "sat" | "saturday" => Ok(Weekday::Sat),
            "sun" | "sunday" => Ok(Weekday::Sun),
            other => bail!("unknown weekday: {other}; use mon/tue/wed/thu/fri/sat/sun"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Weekday::Mon => "Mon",
            Weekday::Tue => "Tue",
            Weekday::Wed => "Wed",
            Weekday::Thu => "Thu",
            Weekday::Fri => "Fri",
            Weekday::Sat => "Sat",
            Weekday::Sun => "Sun",
        }
    }
}

impl std::fmt::Display for Weekday {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Date arithmetic (no chrono — Zeller / proleptic Gregorian)
// ---------------------------------------------------------------------------

/// Parse "YYYY-MM-DD" → (year, month, day).
fn parse_ymd(date: &str) -> Result<(i64, u32, u32)> {
    let parts: Vec<&str> = date.splitn(3, '-').collect();
    if parts.len() != 3 {
        bail!("invalid date: {date}");
    }
    let y: i64 = parts[0]
        .parse()
        .with_context(|| format!("bad year in {date}"))?;
    let m: u32 = parts[1]
        .parse()
        .with_context(|| format!("bad month in {date}"))?;
    let d: u32 = parts[2]
        .parse()
        .with_context(|| format!("bad day in {date}"))?;
    Ok((y, m, d))
}

/// Format (year, month, day) → "YYYY-MM-DD".
fn fmt_ymd(y: i64, m: u32, d: u32) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days in a month (proleptic Gregorian). Used in tests.
#[cfg(test)]
fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Convert (y, m, d) → Julian Day Number (for arithmetic only).
fn to_jdn(y: i64, m: u32, d: u32) -> i64 {
    let m = m as i64;
    let d = d as i64;
    let a = (14 - m) / 12;
    let yy = y + 4800 - a;
    let mm = m + 12 * a - 3;
    d + (153 * mm + 2) / 5 + 365 * yy + yy / 4 - yy / 100 + yy / 400 - 32045
}

/// Convert Julian Day Number → (y, m, d).
fn from_jdn(jdn: i64) -> (i64, u32, u32) {
    let a = jdn + 32044;
    let b = (4 * a + 3) / 146097;
    let c = a - 146097 * b / 4;
    let dd = (4 * c + 3) / 1461;
    let e = c - 1461 * dd / 4;
    let mm = (5 * e + 2) / 153;
    let day = (e - (153 * mm + 2) / 5 + 1) as u32;
    let month = (mm + 3 - 12 * (mm / 10)) as u32;
    let year = 100 * b + dd - 4800 + mm / 10;
    (year, month, day)
}

/// Add `n` days to a "YYYY-MM-DD" date string.
fn add_days(date: &str, n: i64) -> Result<String> {
    let (y, m, d) = parse_ymd(date)?;
    let jdn = to_jdn(y, m, d) + n;
    let (ny, nm, nd) = from_jdn(jdn);
    Ok(fmt_ymd(ny, nm, nd))
}

/// Return the weekday for a "YYYY-MM-DD" date string.
pub fn weekday_of(date: &str) -> Result<Weekday> {
    let (y, m, d) = parse_ymd(date)?;
    let jdn = to_jdn(y, m, d);
    // JDN 0 is a Monday in proleptic calendar: JDN % 7, adjusted so Mon=0
    let idx = ((jdn % 7) + 7) as u32 % 7;
    // JDN=0 → day-of-week for proleptic Gregorian: 0=Mon via ISO-8601
    // Verify: 2026-06-15 (known Monday). to_jdn(2026,6,15) = ?
    // We'll use the standard: JDN mod 7 where JDN=0 is Monday (ISO).
    Ok(Weekday::from_index(idx))
}

/// Find the next date (inclusive of `from_date`) that is one of the given weekdays.
fn next_day_in(from_date: &str, days: &[Weekday]) -> Result<String> {
    if days.is_empty() {
        bail!("no weekdays specified");
    }
    let mut date = from_date.to_string();
    for _ in 0..7 {
        let wd = weekday_of(&date)?;
        if days.contains(&wd) {
            return Ok(date);
        }
        date = add_days(&date, 1)?;
    }
    bail!("could not find a matching weekday within 7 days")
}

/// Find the next date (exclusive of `from_date`, i.e. strictly after) that is one of the given weekdays.
fn next_day_after(from_date: &str, days: &[Weekday]) -> Result<String> {
    let tomorrow = add_days(from_date, 1)?;
    next_day_in(&tomorrow, days)
}

// ---------------------------------------------------------------------------
// Cadence
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cadence {
    pub session_days: Vec<Weekday>,
    pub bind_day: Weekday,
    pub games_enabled: bool,
    pub digest_day: Weekday,
}

impl Default for Cadence {
    fn default() -> Self {
        Cadence {
            session_days: vec![
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
            ],
            bind_day: Weekday::Sun,
            games_enabled: true,
            digest_day: Weekday::Mon,
        }
    }
}

impl Cadence {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading cadence config at {}", path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("parsing cadence config at {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating dir {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self).context("serialising cadence")?;
        std::fs::write(path, json).with_context(|| format!("writing cadence to {}", path.display()))
    }

    /// True if `date` (YYYY-MM-DD) is a session day.
    pub fn is_session_day(&self, date: &str) -> bool {
        weekday_of(date)
            .map(|wd| self.session_days.contains(&wd))
            .unwrap_or(false)
    }

    /// True if `date` is the bind day.
    pub fn is_bind_day(&self, date: &str) -> bool {
        weekday_of(date)
            .map(|wd| wd == self.bind_day)
            .unwrap_or(false)
    }

    /// True if `date` is the digest day.
    pub fn is_digest_day(&self, date: &str) -> bool {
        weekday_of(date)
            .map(|wd| wd == self.digest_day)
            .unwrap_or(false)
    }

    /// Next session day on or after `from_date`.
    pub fn next_session(&self, from_date: &str) -> Result<String> {
        next_day_in(from_date, &self.session_days)
    }

    /// Next bind day strictly after `from_date`.
    pub fn next_bind(&self, from_date: &str) -> Result<String> {
        next_day_after(from_date, &[self.bind_day])
    }

    /// Next digest day strictly after `from_date`.
    pub fn next_digest(&self, from_date: &str) -> Result<String> {
        next_day_after(from_date, &[self.digest_day])
    }
}

/// Default path for the cadence config.
pub fn default_cadence_path() -> std::path::PathBuf {
    let xdg = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
        format!(
            "{}/.local/share",
            std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
        )
    });
    std::path::PathBuf::from(xdg)
        .join("roundtable")
        .join("cadence.json")
}

/// Load cadence from the default path, or return the default if the file doesn't exist.
pub fn load_or_default() -> Result<Cadence> {
    let path = default_cadence_path();
    if path.exists() {
        Cadence::from_file(&path)
    } else {
        Ok(Cadence::default())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_cadence_has_five_session_days() {
        let c = Cadence::default();
        assert_eq!(c.session_days.len(), 5);
        for wd in &[
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
        ] {
            assert!(c.session_days.contains(wd), "{wd} should be a session day");
        }
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
    fn test_next_session_from_friday_gives_monday() {
        let c = Cadence::default();
        // 2026-06-19 is a Friday
        let next = c.next_session("2026-06-20").unwrap(); // Saturday → next is Monday
        assert_eq!(next, "2026-06-22", "next session after Saturday should be Monday 2026-06-22");
    }

    #[test]
    fn test_next_session_from_friday_same_day() {
        let c = Cadence::default();
        // 2026-06-19 is a Friday — next_session on that day returns same day
        let next = c.next_session("2026-06-19").unwrap();
        assert_eq!(next, "2026-06-19");
    }

    #[test]
    fn test_cadence_roundtrips_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("cadence.json");
        let c = Cadence::default();
        c.save(&path).unwrap();
        let loaded = Cadence::from_file(&path).unwrap();
        assert_eq!(loaded.session_days.len(), c.session_days.len());
        assert_eq!(loaded.bind_day, c.bind_day);
        assert_eq!(loaded.digest_day, c.digest_day);
        assert_eq!(loaded.games_enabled, c.games_enabled);
    }

    #[test]
    fn test_cadence_set_updates_session_days() {
        let mut c = Cadence::default();
        c.session_days = vec![Weekday::Mon, Weekday::Wed, Weekday::Fri];
        assert_eq!(c.session_days.len(), 3);
        assert!(c.session_days.contains(&Weekday::Mon));
        assert!(c.session_days.contains(&Weekday::Wed));
        assert!(c.session_days.contains(&Weekday::Fri));
        assert!(!c.session_days.contains(&Weekday::Tue));
    }

    #[test]
    fn test_weekday_of_known_dates() {
        // 2026-06-15 is Monday
        assert_eq!(weekday_of("2026-06-15").unwrap(), Weekday::Mon);
        // 2026-06-16 is Tuesday
        assert_eq!(weekday_of("2026-06-16").unwrap(), Weekday::Tue);
        // 2026-06-14 is Sunday
        assert_eq!(weekday_of("2026-06-14").unwrap(), Weekday::Sun);
        // 2026-06-13 is Saturday
        assert_eq!(weekday_of("2026-06-13").unwrap(), Weekday::Sat);
        // 2026-06-19 is Friday
        assert_eq!(weekday_of("2026-06-19").unwrap(), Weekday::Fri);
    }

    #[test]
    fn test_next_bind_from_saturday_gives_sunday() {
        let c = Cadence::default();
        // 2026-06-13 is Saturday, next bind (Sunday) is 2026-06-14
        let next = c.next_bind("2026-06-13").unwrap();
        assert_eq!(next, "2026-06-14");
    }

    #[test]
    fn test_next_digest_from_saturday_gives_monday() {
        let c = Cadence::default();
        // 2026-06-13 is Saturday, next digest (Monday) is 2026-06-15
        let next = c.next_digest("2026-06-13").unwrap();
        assert_eq!(next, "2026-06-15");
    }

    #[test]
    fn test_days_in_month_leap() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
    }

    #[test]
    fn test_add_days_wraps_month() {
        assert_eq!(add_days("2026-01-31", 1).unwrap(), "2026-02-01");
        assert_eq!(add_days("2026-12-31", 1).unwrap(), "2027-01-01");
    }
}

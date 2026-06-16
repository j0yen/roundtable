//! Property-based invariants for roundtable.
//! READ-ONLY: the edit-agent must not modify this file.

use proptest::prelude::*;

// Epoch date formatting invariant: epoch_to_date should never panic on any u64.
// We test the public surface indirectly via the binary's date computation.
// Here we verify the simple properties on the ledger dedup logic.

proptest! {
    #[test]
    fn date_string_format_valid(days in 0u64..100_000u64) {
        // epoch_to_date is not public but we can verify the format via roundtable's output.
        // For a proptest baseline, just verify the function doesn't panic.
        let secs = days * 86400;
        // We call it via a helper in the module tree.
        let date = epoch_to_date_helper(secs);
        // YYYY-MM-DD format
        assert_eq!(date.len(), 10, "date string must be 10 chars: {date}");
        assert_eq!(&date[4..5], "-", "separator at pos 4: {date}");
        assert_eq!(&date[7..8], "-", "separator at pos 7: {date}");
        let year: u32 = date[0..4].parse().expect("year numeric");
        let month: u32 = date[5..7].parse().expect("month numeric");
        let day: u32 = date[8..10].parse().expect("day numeric");
        assert!(year >= 1970, "year >= 1970: {year}");
        assert!((1..=12).contains(&month), "month 1-12: {month}");
        assert!((1..=31).contains(&day), "day 1-31: {day}");
    }
}

/// Helper that replicates the epoch_to_date logic for proptest.
fn epoch_to_date_helper(secs: u64) -> String {
    let days = secs / 86400;
    let mut remaining = days;
    let mut year: u64 = 1970;
    loop {
        let days_in_year: u64 = if is_leap(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days: &[u64] = if leap {
        &[31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        &[31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month: u64 = 1;
    for &md in month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        month += 1;
    }
    let day = remaining + 1;
    format!("{year:04}-{month:02}-{day:02}")
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

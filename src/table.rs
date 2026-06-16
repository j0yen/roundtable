//! Parse `table.json` produced by `the-lunch`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

/// The table.json top-level type.
#[derive(Debug, Deserialize)]
pub struct Table {
    /// All dishes, including empty seats.
    pub dishes: Vec<Dish>,
}

/// A single creative artifact on the table.
#[derive(Debug, Deserialize)]
pub struct Dish {
    /// Whether this seat was actually filled.
    pub present: bool,
    /// Where it came from on disk.
    pub provenance: Provenance,
}

/// Source location for a dish.
#[derive(Debug, Deserialize)]
pub struct Provenance {
    /// Path to the specific artifact file, if known.
    pub artifact_path: Option<PathBuf>,
}

impl Table {
    /// Parse a table from a JSON file path.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or is not valid JSON.
    pub fn from_path(path: &std::path::Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("reading table.json at {}", path.display()))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("parsing table.json at {}", path.display()))
    }

    /// Return only the dishes that are present and have a known artifact path.
    #[must_use]
    pub fn present_artifacts(&self) -> Vec<&PathBuf> {
        self.dishes
            .iter()
            .filter(|d| d.present)
            .filter_map(|d| d.provenance.artifact_path.as_ref())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_tmp(json: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("tmp file");
        f.write_all(json.as_bytes()).expect("write");
        f
    }

    #[test]
    fn parse_present_artifacts() {
        let json = r#"{
            "date": "2026-06-15",
            "created": "2026-06-15T00:00:00+00:00",
            "dishes": [
                {"present": true, "provenance": {"artifact_path": "/tmp/a.txt"}, "kind": "Haiku", "source": "haiku", "title": null, "content": "x"},
                {"present": false, "provenance": {"artifact_path": "/tmp/b.txt"}, "kind": "Letter", "source": "letter", "title": null, "content": "y"},
                {"present": true, "provenance": {"artifact_path": null}, "kind": "AmbientCue", "source": "ambient", "title": null, "content": "z"}
            ]
        }"#;
        let f = write_tmp(json);
        let table = Table::from_path(f.path()).expect("parse");
        let artifacts = table.present_artifacts();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].to_string_lossy(), "/tmp/a.txt");
    }

    #[test]
    fn empty_dishes_returns_empty() {
        let json = r#"{"date": "2026-06-15", "created": "2026-06-15T00:00:00+00:00", "dishes": []}"#;
        let f = write_tmp(json);
        let table = Table::from_path(f.path()).expect("parse");
        assert!(table.present_artifacts().is_empty());
    }
}

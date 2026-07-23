//! The transaction journal: an append-only record of applied fixes that
//! powers `undo`. Stored as JSON at a path the caller chooses
//! (`/var/lib/sysmedic/journal.json` for the privileged helper).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use sysmedic_core::fix::FixCommand;

/// One applied fix, with everything needed to undo it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JournalEntry {
    pub fix_id: String,
    pub title: String,
    pub applied_at: String,
    pub reversible: bool,
    pub undo: Vec<FixCommand>,
    /// Set once the entry has been undone, so it is not undone twice.
    #[serde(default)]
    pub undone: bool,
}

/// The persisted list of applied fixes.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Journal {
    #[serde(skip)]
    path: PathBuf,
    entries: Vec<JournalEntry>,
}

impl Journal {
    /// Load the journal at `path`, or an empty one if it does not exist.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let entries = match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw)
                .map_err(|e| format!("corrupt journal at {}: {e}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
        };
        Ok(Journal { path, entries })
    }

    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    /// Append an entry and persist.
    pub fn record(&mut self, entry: JournalEntry) -> Result<(), String> {
        self.entries.push(entry);
        self.save()
    }

    /// The most recent entry that can still be undone.
    pub fn last_undoable(&self) -> Option<(usize, &JournalEntry)> {
        self.entries
            .iter()
            .enumerate()
            .rev()
            .find(|(_, e)| e.reversible && !e.undone)
    }

    /// Mark the entry at `index` undone and persist.
    pub fn mark_undone(&mut self, index: usize) -> Result<(), String> {
        if let Some(entry) = self.entries.get_mut(index) {
            entry.undone = true;
        }
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(&self.entries).expect("entries serialize");
        std::fs::write(&self.path, json)
            .map_err(|e| format!("cannot write {}: {e}", self.path.display()))
    }
}

/// An RFC3339 timestamp for `applied_at`.
pub fn now_rfc3339() -> String {
    humantime::format_rfc3339_seconds(SystemTime::now()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, reversible: bool) -> JournalEntry {
        JournalEntry {
            fix_id: id.into(),
            title: id.into(),
            applied_at: "2026-07-23T00:00:00Z".into(),
            reversible,
            undo: if reversible {
                vec![FixCommand::new("ufw", &["disable"])]
            } else {
                vec![]
            },
            undone: false,
        }
    }

    #[test]
    fn records_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sysmedic/journal.json");
        let mut j = Journal::load(&path).unwrap();
        j.record(entry("fix.enable_ufw", true)).unwrap();
        let reloaded = Journal::load(&path).unwrap();
        assert_eq!(reloaded.entries().len(), 1);
        assert_eq!(reloaded.entries()[0].fix_id, "fix.enable_ufw");
    }

    #[test]
    fn last_undoable_skips_irreversible_and_undone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.json");
        let mut j = Journal::load(&path).unwrap();
        j.record(entry("fix.enable_ufw", true)).unwrap();
        j.record(entry("fix.apt_clean", false)).unwrap();
        // Most recent undoable is the ufw entry, not the irreversible clean.
        let (idx, e) = j.last_undoable().unwrap();
        assert_eq!(e.fix_id, "fix.enable_ufw");
        j.mark_undone(idx).unwrap();
        // Now nothing is left to undo.
        assert!(j.last_undoable().is_none());
    }

    #[test]
    fn missing_file_loads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::load(dir.path().join("nope.json")).unwrap();
        assert!(j.entries().is_empty());
    }
}

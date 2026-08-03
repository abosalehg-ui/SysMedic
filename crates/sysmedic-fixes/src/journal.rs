//! The transaction journal: an append-only record of applied fixes that
//! powers `undo`. Stored as JSON at a path the caller chooses
//! (`/var/lib/sysmedic/journal.json` for the privileged helper).

use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use sysmedic_core::fix::FixCommand;

/// Why a journal operation failed. Typed so callers can distinguish a corrupt
/// file (user intervention needed) from plain I/O trouble.
#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("corrupt journal at {path}: {source}")]
    Corrupt {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("cannot access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// How far an entry got. Written **before** the fix runs, so a crash or a
/// failed journal write can never leave a change on the system that the
/// journal has no record of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EntryState {
    /// Recorded, but the commands had not finished when this was written.
    /// Either they are still running, or the process died partway.
    Pending,
    /// Every command completed successfully.
    #[default]
    Applied,
    /// A command failed; the fix may be partially applied.
    Failed,
}

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
    /// Defaults to `Applied` so journals written by older versions (which had
    /// no such field, and only ever recorded completed fixes) still load.
    #[serde(default)]
    pub state: EntryState,
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
    pub fn load(path: impl AsRef<Path>) -> Result<Self, JournalError> {
        let path = path.as_ref().to_path_buf();
        let entries = match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).map_err(|e| JournalError::Corrupt {
                path: path.clone(),
                source: e,
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(JournalError::Io { path, source: e }),
        };
        Ok(Journal { path, entries })
    }

    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    /// Append an entry and persist.
    pub fn record(&mut self, entry: JournalEntry) -> Result<(), JournalError> {
        self.entries.push(entry);
        self.save()
    }

    /// Record the *intent* to apply a fix, before any command runs, and return
    /// the entry's index.
    ///
    /// This is the write-ahead half of the apply protocol. Recording after the
    /// commands ran meant a failing journal write (a full `/var`, which is a
    /// very live possibility in a tool people reach for *because* their disk
    /// is full) left the system changed with nothing to undo it — the user saw
    /// an error and reasonably concluded nothing had happened.
    pub fn record_pending(&mut self, entry: JournalEntry) -> Result<usize, JournalError> {
        self.entries.push(JournalEntry {
            state: EntryState::Pending,
            ..entry
        });
        self.save()?;
        Ok(self.entries.len() - 1)
    }

    /// Move the entry at `index` to its terminal state and persist.
    pub fn finish(&mut self, index: usize, state: EntryState) -> Result<(), JournalError> {
        if let Some(entry) = self.entries.get_mut(index) {
            entry.state = state;
        }
        self.save()
    }

    /// The most recent entry that can still be undone.
    ///
    /// A `Failed` or still-`Pending` entry is undoable too: those are exactly
    /// the cases where the system may have been changed halfway and the user
    /// most needs a way back. Only an already-undone or irreversible entry is
    /// skipped.
    pub fn last_undoable(&self) -> Option<(usize, &JournalEntry)> {
        self.entries
            .iter()
            .enumerate()
            .rev()
            .find(|(_, e)| e.reversible && !e.undone)
    }

    /// Mark the entry at `index` undone and persist.
    pub fn mark_undone(&mut self, index: usize) -> Result<(), JournalError> {
        if let Some(entry) = self.entries.get_mut(index) {
            entry.undone = true;
        }
        self.save()
    }

    fn save(&self) -> Result<(), JournalError> {
        let io = |p: &Path| {
            let path = p.to_path_buf();
            move |e: std::io::Error| JournalError::Io { path, source: e }
        };
        let dir = self.path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(dir).map_err(io(dir))?;
        // Lock the state directory down to its owner. For the privileged
        // helper this is `/var/lib/sysmedic`, owned by root — so an
        // unprivileged user cannot plant or rewrite `journal.json`, whose
        // contents drive `undo` as root. Best-effort on exotic filesystems.
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));

        let json = serde_json::to_string_pretty(&self.entries).expect("entries serialize");

        // Write to a sibling temp file with 0600 and O_NOFOLLOW, then rename
        // into place. The rename is atomic (readers never see a half-written
        // journal) and O_NOFOLLOW refuses a pre-planted symlink at the temp
        // path, closing the TOCTOU/symlink-overwrite window.
        let tmp = dir.join(format!(".journal.{}.tmp", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&tmp)
            .map_err(io(&tmp))?;
        file.write_all(json.as_bytes()).map_err(io(&tmp))?;
        drop(file);
        std::fs::rename(&tmp, &self.path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            JournalError::Io {
                path: self.path.clone(),
                source: e,
            }
        })
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
            state: EntryState::Applied,
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

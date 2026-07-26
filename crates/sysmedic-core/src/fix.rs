//! The safe-fix contract.
//!
//! SysMedic never executes a fix silently. Every fix first presents a
//! [`FixPlan`] answering the three questions the user is owed before any
//! change: *what will happen*, *can it be undone*, and *which files/commands
//! are involved*. Execution lives in the `sysmedic-fixes` crate; this module
//! is pure data so the same plan drives both the preview and the run.

use serde::{Deserialize, Serialize};

use crate::finding::Severity;
use crate::lang::Lang;

/// A single command a fix would run. Structured (program + args, never a
/// shell string) so it is safe to execute without a shell and easy to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl FixCommand {
    pub fn new(program: &str, args: &[&str]) -> Self {
        FixCommand {
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
        }
    }

    /// Human-readable, copy-pasteable rendering of the command.
    pub fn display(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

/// The full preview of a fix: what runs, what it touches, and how to undo it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixPlan {
    /// Stable fix id, e.g. `fix.apt_clean`.
    pub id: String,
    /// Short human title.
    pub title: String,
    /// What will happen, in plain language.
    pub description: String,
    /// Exact commands that would run, in order.
    pub commands: Vec<FixCommand>,
    /// Files and directories the fix creates, modifies or removes.
    pub affected_paths: Vec<String>,
    /// Whether SysMedic can undo this fix from its transaction journal.
    pub reversible: bool,
    /// Commands that undo the fix, when `reversible` (empty otherwise).
    pub undo: Vec<FixCommand>,
    /// Residual risk of applying the fix.
    pub risk: Severity,
    /// Whether the fix needs root (nearly always true).
    pub needs_root: bool,
}

/// The field labels of a fix-plan preview, per language. The preview is the
/// consent step for a privileged, possibly irreversible change — a user must
/// be able to read it in their own language.
struct PreviewLabels {
    risk: &'static str,
    reversible: &'static str,
    yes: &'static str,
    no: &'static str,
    commands: &'static str,
    affected_paths: &'static str,
    undo: &'static str,
}

fn preview_labels(lang: Lang) -> PreviewLabels {
    match lang {
        Lang::En => PreviewLabels {
            risk: "Risk",
            reversible: "Reversible",
            yes: "yes",
            no: "no",
            commands: "Commands that will run:",
            affected_paths: "Affected paths:",
            undo: "Undo:",
        },
        Lang::Ar => PreviewLabels {
            risk: "الخطورة",
            reversible: "قابل للتراجع",
            yes: "نعم",
            no: "لا",
            commands: "الأوامر التي ستُنفَّذ:",
            affected_paths: "المسارات المتأثرة:",
            undo: "أوامر التراجع:",
        },
    }
}

impl FixPlan {
    /// Render the plan as the confirmation text a user must read before
    /// approving: what happens, reversibility, commands and affected paths.
    pub fn preview(&self) -> String {
        self.preview_in(Lang::En)
    }

    /// [`FixPlan::preview`] with localized field labels. (The plan title and
    /// description come from the fix registry and are English for now; the
    /// labels around the consent-critical facts — risk, reversibility, exact
    /// commands — follow the user's language.)
    pub fn preview_in(&self, lang: Lang) -> String {
        use std::fmt::Write as _;
        let l = preview_labels(lang);
        let mut out = String::new();
        let _ = writeln!(out, "{}", self.title);
        let _ = writeln!(out, "{}\n", self.description);
        let _ = writeln!(out, "{}: {}", l.risk, self.risk.label_in(lang));
        let _ = writeln!(
            out,
            "{}: {}",
            l.reversible,
            if self.reversible { l.yes } else { l.no }
        );
        if !self.commands.is_empty() {
            let _ = writeln!(out, "\n{}", l.commands);
            for c in &self.commands {
                let _ = writeln!(out, "  $ {}", c.display());
            }
        }
        if !self.affected_paths.is_empty() {
            let _ = writeln!(out, "\n{}", l.affected_paths);
            for p in &self.affected_paths {
                let _ = writeln!(out, "  - {p}");
            }
        }
        if self.reversible && !self.undo.is_empty() {
            let _ = writeln!(out, "\n{}", l.undo);
            for c in &self.undo {
                let _ = writeln!(out, "  $ {}", c.display());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_display_roundtrips() {
        let c = FixCommand::new("apt-get", &["clean"]);
        assert_eq!(c.display(), "apt-get clean");
        assert_eq!(FixCommand::new("snap", &[]).display(), "snap");
    }

    #[test]
    fn preview_states_reversibility_and_commands() {
        let plan = FixPlan {
            id: "fix.enable_ufw".into(),
            title: "Enable the firewall".into(),
            description: "Turns on ufw.".into(),
            commands: vec![FixCommand::new("ufw", &["--force", "enable"])],
            affected_paths: vec!["/etc/ufw".into()],
            reversible: true,
            undo: vec![FixCommand::new("ufw", &["disable"])],
            risk: Severity::Low,
            needs_root: true,
        };
        let preview = plan.preview();
        assert!(preview.contains("Reversible: yes"));
        assert!(preview.contains("ufw --force enable"));
        assert!(preview.contains("ufw disable"));

        // The Arabic preview keeps the exact commands but localizes the
        // consent labels.
        let ar = plan.preview_in(Lang::Ar);
        assert!(ar.contains("قابل للتراجع: نعم"));
        assert!(ar.contains("ufw --force enable"));
        assert!(!ar.contains("Reversible"));
    }
}

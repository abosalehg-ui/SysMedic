//! `sysmedic schedule` — periodic checkups via **systemd user timers**.
//!
//! Rather than run a resident polling daemon, SysMedic installs a systemd
//! *user* timer that runs `sysmedic monitor` on a schedule. This is the
//! Linux-native way to schedule work: it survives reboots, costs nothing
//! while idle, and is battery-friendly. The unit-file builders below are
//! pure and unit-tested; installation just writes them and calls
//! `systemctl --user`.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use owo_colors::OwoColorize;

const SERVICE: &str = "sysmedic-checkup.service";
const TIMER: &str = "sysmedic-checkup.timer";

#[derive(Clone, Copy, PartialEq)]
pub enum Cadence {
    Daily,
    Weekly,
    Monthly,
}

impl Cadence {
    fn on_calendar(self) -> &'static str {
        match self {
            // Slightly off the hour so many machines don't fire at once.
            Cadence::Daily => "*-*-* 09:17:00",
            Cadence::Weekly => "Mon *-*-* 09:17:00",
            Cadence::Monthly => "*-*-01 09:17:00",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Cadence::Daily => "daily",
            Cadence::Weekly => "weekly",
            Cadence::Monthly => "monthly",
        }
    }
}

/// The `.service` unit that runs one checkup.
pub fn service_unit(exe: &str) -> String {
    // The executable path is quoted so a path containing spaces (e.g. under
    // `~/My Apps/`) still produces a valid `ExecStart`. systemd unquotes it
    // back into a single argv[0].
    format!(
        "[Unit]\n\
         Description=SysMedic scheduled checkup\n\n\
         [Service]\n\
         Type=oneshot\n\
         ExecStart=\"{exe}\" monitor\n"
    )
}

/// The `.timer` unit that triggers the service on `cadence`.
pub fn timer_unit(cadence: Cadence) -> String {
    // `RandomizedDelaySec` actually spreads the load across machines — the base
    // `OnCalendar` time is identical everywhere, so without this every host
    // would fire at the same instant.
    format!(
        "[Unit]\n\
         Description=SysMedic {} checkup timer\n\n\
         [Timer]\n\
         OnCalendar={}\n\
         RandomizedDelaySec=1h\n\
         Persistent=true\n\n\
         [Install]\n\
         WantedBy=timers.target\n",
        cadence.label(),
        cadence.on_calendar()
    )
}

fn user_unit_dir() -> Result<PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".config"))
        })
        .context("cannot determine config dir (no HOME)")?;
    Ok(base.join("systemd/user"))
}

fn exe() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| "sysmedic".to_string())
}

fn systemctl(args: &[&str]) -> Result<()> {
    let status = std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()
        .context("failed to run systemctl --user")?;
    if !status.success() {
        bail!("systemctl --user {} failed", args.join(" "));
    }
    Ok(())
}

/// Install the timer for `cadence`.
pub fn enable(cadence: Cadence) -> Result<()> {
    let dir = user_unit_dir()?;
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::write(dir.join(SERVICE), service_unit(&exe()))?;
    std::fs::write(dir.join(TIMER), timer_unit(cadence))?;

    systemctl(&["daemon-reload"]).ok();
    match systemctl(&["enable", "--now", TIMER]) {
        Ok(()) => println!(
            "{} Scheduled {} checkups. See status with `systemctl --user list-timers`.",
            "✓".green(),
            cadence.label()
        ),
        Err(_) => println!(
            "{} Installed the {} timer units, but could not activate them here \
             (no user systemd session). On your desktop run:\n    systemctl --user enable --now {}",
            "!".yellow(),
            cadence.label(),
            TIMER
        ),
    }
    Ok(())
}

/// Remove the timer.
pub fn disable() -> Result<()> {
    systemctl(&["disable", "--now", TIMER]).ok();
    let dir = user_unit_dir()?;
    let _ = std::fs::remove_file(dir.join(TIMER));
    let _ = std::fs::remove_file(dir.join(SERVICE));
    systemctl(&["daemon-reload"]).ok();
    println!("{} Scheduled checkups disabled.", "✓".green());
    Ok(())
}

/// Show whether the timer is installed and when it next runs.
pub fn status() -> Result<()> {
    let installed = user_unit_dir()
        .map(|d| d.join(TIMER).exists())
        .unwrap_or(false);
    if !installed {
        println!(
            "Scheduled checkups: {}. Enable with `sysmedic schedule daily`.",
            "off".yellow()
        );
        return Ok(());
    }
    println!("Scheduled checkups: {}", "on".green());
    // Best-effort: show the next run time.
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "list-timers", TIMER, "--no-pager"])
        .status();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_unit_runs_monitor() {
        let unit = service_unit("/usr/bin/sysmedic");
        assert!(unit.contains("ExecStart=\"/usr/bin/sysmedic\" monitor"));
        assert!(unit.contains("Type=oneshot"));
    }

    #[test]
    fn service_unit_quotes_paths_with_spaces() {
        let unit = service_unit("/home/a/My Apps/sysmedic");
        assert!(unit.contains("ExecStart=\"/home/a/My Apps/sysmedic\" monitor"));
    }

    #[test]
    fn timer_units_have_expected_schedule() {
        assert!(timer_unit(Cadence::Daily).contains("OnCalendar=*-*-* 09:17:00"));
        assert!(timer_unit(Cadence::Weekly).contains("OnCalendar=Mon *-*-* 09:17:00"));
        assert!(timer_unit(Cadence::Monthly).contains("OnCalendar=*-*-01 09:17:00"));
        assert!(timer_unit(Cadence::Daily).contains("WantedBy=timers.target"));
        // Load is spread across machines rather than all firing at 09:17.
        assert!(timer_unit(Cadence::Daily).contains("RandomizedDelaySec="));
    }
}

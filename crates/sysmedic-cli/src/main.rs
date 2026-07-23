mod fix;
mod schedule;
mod text;
mod tools;

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use sysmedic_core::Engine;
use sysmedic_knowledge::Lang;

#[derive(Parser)]
#[command(
    name = "sysmedic",
    version,
    about = "SysMedic — a doctor for your Linux system: checkup, diagnose, explain, prescribe.",
    after_help = "Author:  abosalehg-ui <ar0.history@gmail.com>\n\
                  Source:  https://github.com/abosalehg-ui/SysMedic\n\
                  Issues:  https://github.com/abosalehg-ui/SysMedic/issues\n\
                  License: GPL-3.0-or-later"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run a full health checkup and print the report (default)
    Checkup {
        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        /// Write the report to a file instead of stdout
        #[arg(long)]
        output: Option<PathBuf>,
        /// Explanation language (defaults to $LANG)
        #[arg(long, value_enum)]
        lang: Option<CliLang>,
    },
    /// List all diagnostic checks SysMedic performs
    Checks,
    /// Explain a finding id (cause, danger, impact, remedy, risk)
    Explain {
        /// Finding id, e.g. storage.disk_nearly_full
        id: String,
        #[arg(long, value_enum)]
        lang: Option<CliLang>,
    },
    /// Preview or apply a safe fix (omit id to list applicable fixes)
    Fix {
        /// Fix id, e.g. fix.apt_clean
        id: Option<String>,
        /// Show the plan without changing anything
        #[arg(long)]
        dry_run: bool,
        /// Apply the fix (otherwise only the preview is shown)
        #[arg(long)]
        yes: bool,
    },
    /// Undo the most recent reversible fix
    Undo {
        /// Perform the undo (otherwise only shows what would be undone)
        #[arg(long)]
        yes: bool,
    },
    /// Analyze disk usage: largest subdirectories of a path (default: cwd)
    Disk {
        /// Directory to scan
        path: Option<String>,
        /// How many levels deep to keep in the tree
        #[arg(long, default_value_t = 4)]
        depth: u32,
        /// How many top entries to show
        #[arg(long, default_value_t = 15)]
        top: usize,
    },
    /// Show network status: route, DNS, listening ports and latency
    Network,
    /// Run a checkup, record it in history, and notify on active alerts
    Monitor {
        /// Suppress stdout (for scheduled runs); still sends notifications
        #[arg(long)]
        quiet: bool,
    },
    /// Show the recorded health-score trend
    History,
    /// Schedule automatic checkups via a systemd user timer
    Schedule {
        #[command(subcommand)]
        action: ScheduleAction,
    },
}

#[derive(Subcommand)]
enum ScheduleAction {
    /// Run a checkup every day
    Daily,
    /// Run a checkup every week
    Weekly,
    /// Run a checkup every month
    Monthly,
    /// Turn off scheduled checkups
    Off,
    /// Show whether scheduled checkups are enabled
    Status,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
    Markdown,
    Html,
    Pdf,
}

#[derive(Clone, Copy, ValueEnum)]
enum CliLang {
    En,
    Ar,
}

fn resolve_lang(cli: Option<CliLang>) -> Lang {
    match cli {
        Some(CliLang::En) => Lang::En,
        Some(CliLang::Ar) => Lang::Ar,
        None => Lang::from_locale(&std::env::var("LANG").unwrap_or_default()),
    }
}

fn engine() -> Engine {
    Engine::new()
        .with_collectors(sysmedic_collectors::default_collectors())
        .with_diagnostics(sysmedic_diagnostics::default_diagnostics())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Checkup {
        format: Format::Text,
        output: None,
        lang: None,
    }) {
        Command::Checkup {
            format,
            output,
            lang,
        } => {
            let lang = resolve_lang(lang);
            eprintln!("Running SysMedic checkup...");
            let report = engine().run();

            if format == Format::Pdf {
                let path = output.unwrap_or_else(|| PathBuf::from("sysmedic-report.pdf"));
                match sysmedic_report::write_pdf(&report, lang, &path) {
                    Ok(()) => eprintln!("PDF report written to {}", path.display()),
                    Err(e) => {
                        // Fall back to an HTML file next to the requested path.
                        let html_path = path.with_extension("html");
                        fs::write(&html_path, sysmedic_report::to_html(&report, lang))?;
                        eprintln!("{e}\nHTML report written to {}", html_path.display());
                    }
                }
                return Ok(());
            }

            let rendered = match format {
                Format::Text => text::render(&report, lang),
                Format::Json => sysmedic_report::to_json(&report),
                Format::Markdown => sysmedic_report::to_markdown(&report, lang),
                Format::Html => sysmedic_report::to_html(&report, lang),
                Format::Pdf => unreachable!("handled above"),
            };
            match output {
                Some(path) => {
                    fs::write(&path, rendered)?;
                    eprintln!("Report written to {}", path.display());
                }
                None => println!("{rendered}"),
            }
        }
        Command::Checks => {
            for name in engine().diagnostic_names() {
                println!("{name}");
            }
        }
        Command::Explain { id, lang } => {
            let lang = resolve_lang(lang);
            match sysmedic_knowledge::explain(&id, lang) {
                Some(exp) => {
                    println!("Cause:          {}", exp.cause);
                    println!("Dangerous?      {}", exp.dangerous);
                    println!("Impact:         {}", exp.impact);
                    println!("Remedy:         {}", exp.remedy);
                    println!("If ignored:     {}", exp.risk_if_ignored);
                }
                None => {
                    anyhow::bail!(
                        "unknown finding id '{id}' — run `sysmedic checkup` to see current findings"
                    );
                }
            }
        }
        Command::Fix { id, dry_run, yes } => match id {
            Some(id) => fix::apply(&id, dry_run, yes)?,
            None => fix::list()?,
        },
        Command::Undo { yes } => fix::undo(yes)?,
        Command::Disk { path, depth, top } => tools::disk(path, depth, top)?,
        Command::Network => tools::network()?,
        Command::Monitor { quiet } => tools::monitor(quiet)?,
        Command::History => tools::history()?,
        Command::Schedule { action } => match action {
            ScheduleAction::Daily => schedule::enable(schedule::Cadence::Daily)?,
            ScheduleAction::Weekly => schedule::enable(schedule::Cadence::Weekly)?,
            ScheduleAction::Monthly => schedule::enable(schedule::Cadence::Monthly)?,
            ScheduleAction::Off => schedule::disable()?,
            ScheduleAction::Status => schedule::status()?,
        },
    }
    Ok(())
}

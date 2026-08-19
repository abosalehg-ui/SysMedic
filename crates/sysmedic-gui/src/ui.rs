//! Widget construction. All strings and styling decisions come from the
//! `viewmodel` module; the checkup runs via `gio::spawn_blocking` so the
//! main loop stays responsive.

use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use sysmedic_core::fix::FixPlan;
use sysmedic_core::HealthReport;
use sysmedic_knowledge::Lang;

use crate::viewmodel::{self, Strings};

const REPO_URL: &str = "https://github.com/abosalehg-ui/SysMedic";
const ISSUES_URL: &str = "https://github.com/abosalehg-ui/SysMedic/issues";

/// A "re-run the checkup" callback, shared so a finished fix can trigger it.
type RefreshFn = Rc<dyn Fn()>;

/// Whether the user asked for reduced motion (GNOME exposes this as the
/// `gtk-enable-animations` setting, which the desktop turns off for
/// `prefers-reduced-motion`). The checkup spinner runs for as long as the scan
/// takes, so honouring this matters more here than for a brief transition.
pub fn animations_enabled() -> bool {
    gtk::Settings::default()
        .map(|s| s.is_gtk_enable_animations())
        .unwrap_or(true)
}

/// Pin the widget direction to match the language SysMedic chose.
///
/// The app picks its strings from the environment itself, while GTK derives
/// layout direction from its own translation of `default:LTR` — two
/// independent decisions. When GTK's locale data is missing (a slim container,
/// a Flatpak without the locale extension, `LANGUAGE` set differently from
/// `LC_ALL`) the two disagree and the result is Arabic text in a left-to-right
/// layout: mirrored margins, the wrong side for every icon and switch. Setting
/// the direction from the same value that chose the strings keeps them in step.
pub fn apply_text_direction(lang: Lang) {
    let direction = match lang {
        Lang::Ar => gtk::TextDirection::Rtl,
        Lang::En => gtk::TextDirection::Ltr,
    };
    gtk::Widget::set_default_direction(direction);
}

pub fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".score-title { font-size: 56px; font-weight: 800; } .badge { padding: 2px 8px; }",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn run_engine() -> HealthReport {
    let report = sysmedic_diagnostics::default_engine().run();
    // Record this checkup so the trend strip has data (best-effort). Throttled
    // so rapid refresh clicks don't flood history and turn the trend into a
    // click-rate graph; scheduled `monitor` runs are far enough apart to record.
    let entry = sysmedic_history::HistoryEntry::from_report(&report);
    if let Some(path) = sysmedic_history::default_path() {
        let _ = sysmedic_history::append_throttled(path, &entry, 300);
    }
    report
}

pub fn build_window(app: &adw::Application) {
    let lang = Lang::from_env();
    apply_text_direction(lang);
    let strings = Strings::for_lang(lang);

    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.set_tooltip_text(Some(strings.run_checkup));
    let spinner = gtk::Spinner::new();

    let header = adw::HeaderBar::new();
    header.pack_start(&refresh);
    header.pack_end(&spinner);

    // Primary menu with an "About SysMedic" entry (author + repo live there).
    let menu = gio::Menu::new();
    menu.append(Some(strings.export_report), Some("app.export"));
    menu.append(Some(strings.about), Some("app.about"));
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .tooltip_text(strings.about)
        .build();
    header.pack_end(&menu_button);

    let clamp = adw::Clamp::builder()
        .maximum_size(760)
        .margin_top(24)
        .margin_bottom(36)
        .margin_start(12)
        .margin_end(12)
        .build();

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build();

    // Two pages: the health Overview and the Disk Usage treemap.
    let stack = adw::ViewStack::new();
    stack
        .add_titled(&scrolled, Some("overview"), strings.overview)
        .set_icon_name(Some("emblem-ok-symbolic"));
    stack
        .add_titled(
            &crate::disk::disk_page(lang),
            Some("disk"),
            strings.disk_usage,
        )
        .set_icon_name(Some("drive-harddisk-symbolic"));

    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    header.set_title_widget(Some(&switcher));

    // A bottom switcher bar that a narrow-width breakpoint reveals (the wide
    // in-header switcher truncates instead of adapting on small windows).
    let switcher_bar = adw::ViewSwitcherBar::builder().stack(&stack).build();

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&stack));
    toolbar_view.add_bottom_bar(&switcher_bar);

    // Toasts confirm fix outcomes without interrupting the flow.
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&toolbar_view));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("SysMedic")
        .default_width(780)
        .default_height(860)
        .width_request(360)
        .height_request(400)
        .content(&toasts)
        .build();

    let breakpoint =
        adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 550sp").unwrap());
    breakpoint.add_setter(&switcher_bar, "reveal", Some(&true.to_value()));
    breakpoint.add_setter(
        &header,
        "title-widget",
        Some(&None::<gtk::Widget>.to_value()),
    );
    window.add_breakpoint(breakpoint);

    // Wire the "app.about" action to show the About dialog.
    let about_action = gio::SimpleAction::new("about", None);
    about_action.connect_activate({
        let window = window.clone();
        move |_, _| show_about(&window, lang)
    });
    app.add_action(&about_action);

    // F5 / Ctrl+R re-run the checkup from the keyboard.
    let refresh_action = gio::SimpleAction::new("refresh", None);
    app.add_action(&refresh_action);
    app.set_accels_for_action("app.refresh", &["F5", "<Ctrl>r"]);

    // The most recent completed report, kept so it can be exported on demand.
    let latest_report: Rc<RefCell<Option<HealthReport>>> = Rc::new(RefCell::new(None));

    install_export_action(app, &window, &toasts, latest_report.clone(), lang);

    // `run_checkup` needs to reference itself so a finished fix can trigger a
    // re-scan. A shared cell breaks the chicken-and-egg of the self-reference.
    let self_ref: Rc<RefCell<Option<RefreshFn>>> = Rc::new(RefCell::new(None));
    let run_checkup: RefreshFn = Rc::new({
        let clamp = clamp.clone();
        let spinner = spinner.clone();
        let refresh = refresh.clone();
        let window = window.clone();
        let toasts = toasts.clone();
        let latest_report = latest_report.clone();
        let self_ref = self_ref.clone();
        move || {
            // A perpetually spinning widget is exactly what reduced-motion
            // users are asking to avoid; the status page below still says the
            // checkup is running, so no information is lost by holding still.
            if animations_enabled() {
                spinner.start();
            }
            spinner.set_visible(animations_enabled());
            refresh.set_sensitive(false);
            clamp.set_child(Some(
                &adw::StatusPage::builder()
                    .title(strings.checking)
                    .icon_name("emblem-synchronizing-symbolic")
                    .build(),
            ));
            let clamp = clamp.clone();
            let spinner = spinner.clone();
            let refresh = refresh.clone();
            let window = window.clone();
            let toasts = toasts.clone();
            let latest_report = latest_report.clone();
            let on_changed = self_ref.borrow().clone();
            glib::spawn_future_local(async move {
                let result = gtk::gio::spawn_blocking(run_engine).await;
                spinner.stop();
                refresh.set_sensitive(true);
                match result {
                    Ok(report) => {
                        *latest_report.borrow_mut() = Some(report.clone());
                        let refresh_cb = on_changed.unwrap_or_else(|| Rc::new(|| {}));
                        clamp.set_child(Some(&report_view(
                            &report, lang, &window, &toasts, refresh_cb,
                        )));
                    }
                    Err(_) => clamp.set_child(Some(
                        &adw::StatusPage::builder()
                            .title(strings.checkup_failed)
                            .icon_name("dialog-error-symbolic")
                            .build(),
                    )),
                }
            });
        }
    });
    *self_ref.borrow_mut() = Some(run_checkup.clone());

    refresh.connect_clicked({
        let run_checkup = run_checkup.clone();
        move |_| run_checkup()
    });
    refresh_action.connect_activate({
        let run_checkup = run_checkup.clone();
        move |_, _| run_checkup()
    });
    run_checkup();

    window.present();
}

/// Which renderer an export path's extension selects. Pure, so the mapping is
/// unit-tested without a file dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Html,
    Markdown,
    Json,
}

/// Pick the export format from a filename. Anything unrecognised becomes HTML,
/// which is the format the dialog proposes by default.
pub fn export_format_for(path: &std::path::Path) -> ExportFormat {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("md") | Some("markdown") => ExportFormat::Markdown,
        Some("json") => ExportFormat::Json,
        _ => ExportFormat::Html,
    }
}

/// Render `report` in the requested export format.
pub fn render_export(report: &HealthReport, format: ExportFormat, lang: Lang) -> String {
    match format {
        ExportFormat::Markdown => sysmedic_report::to_markdown(report, lang),
        ExportFormat::Json => sysmedic_report::to_json(report),
        ExportFormat::Html => sysmedic_report::to_html(report, lang),
    }
}

/// "Export report…" — save the last checkup as HTML (or Markdown/JSON, picked
/// by the chosen file extension). Extracted from `build_window`, which had
/// accumulated action wiring, fix handling and layout in one long function.
fn install_export_action(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    toasts: &adw::ToastOverlay,
    latest_report: Rc<RefCell<Option<HealthReport>>>,
    lang: Lang,
) {
    let strings = Strings::for_lang(lang);
    let export_action = gio::SimpleAction::new("export", None);
    export_action.connect_activate({
        let window = window.clone();
        let toasts = toasts.clone();
        move |_, _| {
            let Some(report) = latest_report.borrow().clone() else {
                toasts.add_toast(adw::Toast::new(strings.export_first));
                return;
            };
            let dialog = gtk::FileDialog::builder()
                .title(strings.export_report)
                .initial_name("sysmedic-report.html")
                .build();
            let toasts = toasts.clone();
            dialog.save(
                Some(&window),
                None::<&gio::Cancellable>,
                move |result: Result<gio::File, glib::Error>| {
                    // A closed dialog is a cancellation, not an error.
                    let Some(path) = result.ok().and_then(|f| f.path()) else {
                        return;
                    };
                    let content = render_export(&report, export_format_for(&path), lang);
                    let message =
                        match sysmedic_core::paths::write_private(&path, content.as_bytes()) {
                            Ok(()) => format!("{} — {}", strings.export_done, path.display()),
                            Err(e) => format!("{}: {e}", strings.export_failed),
                        };
                    toasts.add_toast(adw::Toast::new(&message));
                },
            );
        }
    });
    app.add_action(&export_action);
    app.set_accels_for_action("app.export", &["<Ctrl>e"]);
}

/// The About dialog: app identity, author, repository and contact.
fn show_about(parent: &adw::ApplicationWindow, lang: Lang) {
    let strings = Strings::for_lang(lang);
    let about = adw::AboutDialog::builder()
        .application_name("SysMedic")
        .application_icon(crate::APP_ID)
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("abosalehg-ui")
        .developers(["abosalehg-ui <ar0.history@gmail.com>"])
        .comments(strings.app_comment)
        .website(REPO_URL)
        .issue_url(ISSUES_URL)
        .support_url(ISSUES_URL)
        .license_type(gtk::License::Gpl30)
        .copyright("© 2026 abosalehg-ui")
        .build();
    about.add_link("Source code", REPO_URL);
    about.present(Some(parent));
}

/// What the UI should do after the helper exits. Pure, so the branch that
/// decides whether a privileged change succeeded — previously buried in a
/// closure and untested — can be exercised directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixOutcome {
    /// The helper reported success: confirm it and re-scan.
    Applied,
    /// pkexec was cancelled, the helper failed, or it is not installed.
    /// The user must be told, not left with a silent re-scan.
    Failed,
}

/// Decide the outcome from the helper's exit status.
pub fn fix_outcome(helper_succeeded: bool) -> FixOutcome {
    if helper_succeeded {
        FixOutcome::Applied
    } else {
        FixOutcome::Failed
    }
}

/// The dialog appearance a plan's reversibility earns. An irreversible
/// privileged change gets the destructive (red) style rather than the
/// encouraging blue "suggested" one.
pub fn response_appearance_for(reversible: bool) -> adw::ResponseAppearance {
    if reversible {
        adw::ResponseAppearance::Suggested
    } else {
        adw::ResponseAppearance::Destructive
    }
}

/// Ask polkit (via pkexec) to run the helper for `fix_id`, then re-scan.
fn confirm_and_apply(
    window: &adw::ApplicationWindow,
    lang: Lang,
    plan: &FixPlan,
    toasts: &adw::ToastOverlay,
    on_changed: RefreshFn,
) {
    let strings = Strings::for_lang(lang);
    let reversibility = if plan.reversible {
        strings.reversible_yes
    } else {
        strings.reversible_no
    };
    let dialog = adw::AlertDialog::new(Some(strings.confirm_fix_title), Some(reversibility));
    // The preview (commands, paths, risk) as a start-aligned monospace block —
    // centered proportional text made command lines hard to scan.
    // `preview_in` now localizes the plan's title and description as well as
    // the field labels, so an Arabic user reads the whole consent text.
    let preview = gtk::Label::new(Some(&plan.preview_in(lang)));
    preview.set_xalign(0.0);
    preview.set_wrap(true);
    preview.add_css_class("monospace");
    preview.set_selectable(true);
    dialog.set_extra_child(Some(&preview));
    dialog.add_response("cancel", strings.cancel);
    dialog.add_response("apply", strings.apply);
    // An irreversible privileged change earns the destructive (red) style,
    // not the encouraging blue "suggested" one.
    dialog.set_response_appearance("apply", response_appearance_for(plan.reversible));
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    let fix_id = plan.id.clone();
    let window = window.clone();
    // Separate handles for the response callback, so the originals stay
    // available for `dialog.present` at the end of this function.
    let cb_window = window.clone();
    let toasts = toasts.clone();
    dialog.connect_response(None, move |_, response| {
        if response != "apply" {
            return;
        }
        let fix_id = fix_id.clone();
        let on_changed = on_changed.clone();
        let window = cb_window.clone();
        let toasts = toasts.clone();
        // Busy feedback while pkexec prompts and the helper runs.
        let running = adw::Toast::new(strings.fix_running);
        running.set_timeout(0); // stays until dismissed below
        toasts.add_toast(running.clone());
        glib::spawn_future_local(async move {
            // The helper chosen here selects the polkit action, so an
            // irreversible purge and a firewall toggle no longer share one
            // generic authorization prompt.
            let helper = sysmedic_fixes::helper_for_fix(&fix_id).unwrap_or_else(|| {
                sysmedic_fixes::helper_path_for(sysmedic_fixes::FixTier::Destructive)
            });
            let id = fix_id.clone();
            // pkexec prompts polkit; the helper does the privileged work.
            let succeeded = gtk::gio::spawn_blocking(move || {
                Command::new("pkexec")
                    .arg(helper)
                    .arg("apply")
                    .arg(&id)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            })
            .await
            .unwrap_or(false);
            running.dismiss();
            if fix_outcome(succeeded) == FixOutcome::Applied {
                // Say it worked — a silent re-scan left users guessing whether
                // the fix ran or the score just changed.
                toasts.add_toast(adw::Toast::new(strings.fix_applied));
            } else {
                // Tell the user why nothing changed instead of a silent re-scan
                // (auth cancelled, fix failed, or the helper isn't installed).
                let error = adw::AlertDialog::new(
                    Some(strings.fix_failed_title),
                    Some(strings.fix_failed_body),
                );
                error.add_response("ok", strings.ok);
                error.present(Some(&window));
            }
            on_changed(); // re-scan so the UI reflects the new state
        });
    });
    dialog.present(Some(&window));
}

fn report_view(
    report: &HealthReport,
    lang: Lang,
    window: &adw::ApplicationWindow,
    toasts: &adw::ToastOverlay,
    on_changed: RefreshFn,
) -> gtk::Box {
    let strings = Strings::for_lang(lang);
    let root = gtk::Box::new(gtk::Orientation::Vertical, 18);

    // Hero: the big score.
    let score = gtk::Label::new(Some(&format!("{}", report.score)));
    score.add_css_class("score-title");
    score.add_css_class(viewmodel::score_css(report.score));
    let grade = gtk::Label::new(Some(&format!(
        "{} · {}/100",
        sysmedic_core::score::grade_label_in(report.score, lang),
        report.score
    )));
    grade.add_css_class("title-2");
    let generated = gtk::Label::new(Some(&local_timestamp(&report.generated_at)));
    generated.add_css_class("dim-label");
    generated.add_css_class("caption");
    let hero_title = gtk::Label::new(Some(strings.health_score));
    hero_title.add_css_class("dim-label");
    for w in [&hero_title, &score, &grade, &generated] {
        root.append(w);
    }

    // What the score is actually based on. Without this, a machine where four
    // collectors found nothing to talk to showed the same confident number as
    // one that was examined end to end.
    if report.coverage.is_partial() {
        let coverage = gtk::Label::new(Some(&format!(
            "{}: {} — {}",
            sysmedic_core::score::coverage_label_in(lang),
            report.coverage.label(),
            strings.coverage_note
        )));
        coverage.add_css_class("dim-label");
        coverage.add_css_class("caption");
        coverage.set_wrap(true);
        root.append(&coverage);
    }

    // History trend strip (populated by past checkups).
    let entries = sysmedic_history::default_path()
        .map(sysmedic_history::load)
        .unwrap_or_default();
    if entries.len() >= 2 {
        let spark = sysmedic_history::sparkline(&entries, 40);
        let trend = sysmedic_history::trend_delta(&entries)
            .map(|d| format!("  ({d:+} since first)"))
            .unwrap_or_default();
        let history = gtk::Label::new(Some(&format!("{spark}{trend}")));
        history.add_css_class("dim-label");
        history.add_css_class("monospace");
        history.set_tooltip_text(Some(strings.history_tooltip));
        root.append(&history);
    }

    // Category scores.
    root.append(&section_label(strings.categories));
    let categories = boxed_list();
    for row in viewmodel::category_rows(report, lang) {
        let action_row = adw::ActionRow::builder().title(row.label).build();
        if !row.measured {
            // No data: say so rather than drawing a full bar, which reads as a
            // clean bill of health for something nobody looked at.
            let value = gtk::Label::new(Some(strings.not_measured));
            value.add_css_class("dim-label");
            action_row.add_suffix(&value);
            action_row.add_css_class("dim-label");
            categories.append(&action_row);
            continue;
        }
        let bar = gtk::LevelBar::for_interval(0.0, 100.0);
        // Offsets give low scores a color signal — without them 76 and 100
        // render identically.
        bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_LOW, 50.0);
        bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_HIGH, 75.0);
        bar.add_offset_value(gtk::LEVEL_BAR_OFFSET_FULL, 100.0);
        bar.set_value(row.score as f64);
        bar.set_width_request(160);
        bar.set_valign(gtk::Align::Center);
        let value = gtk::Label::new(Some(&row.score.to_string()));
        value.add_css_class("numeric");
        action_row.add_suffix(&bar);
        action_row.add_suffix(&value);
        categories.append(&action_row);
    }
    root.append(&categories);

    // Findings.
    root.append(&section_label(strings.findings));
    let findings = boxed_list();
    if report.findings.is_empty() {
        let ok = adw::ActionRow::builder().title(strings.no_findings).build();
        ok.add_prefix(&gtk::Image::from_icon_name("emblem-ok-symbolic"));
        findings.append(&ok);
    }
    for finding in &report.findings {
        let row = adw::ExpanderRow::builder()
            .title(glib::markup_escape_text(
                &sysmedic_knowledge::localized_title(finding, lang),
            ))
            .subtitle(glib::markup_escape_text(
                &sysmedic_knowledge::localized_summary(finding, lang),
            ))
            .build();
        // The user-facing label, not the machine one — an Arabic dashboard
        // used to show `CRITICAL` above an Arabic title. `Severity::label()`
        // still supplies the CSS class below.
        let badge = gtk::Label::new(Some(&finding.severity.label_in(lang).to_uppercase()));
        badge.add_css_class("badge");
        badge.add_css_class("caption-heading");
        badge.add_css_class(viewmodel::severity_css(finding.severity));
        row.add_prefix(&badge);

        for (question, answer) in viewmodel::explanation_lines(&finding.id, lang) {
            row.add_row(&detail_row(question, &answer));
        }
        if !finding.evidence.is_empty() {
            row.add_row(&detail_row(strings.evidence, &finding.evidence.join("\n")));
        }
        if let Some(hint) = &finding.fix_hint {
            let hint_row = detail_row(strings.suggested_command, hint);
            hint_row.add_css_class("monospace");
            row.add_row(&hint_row);
        }

        // If SysMedic has a one-click fix for this finding, offer it.
        if let Some(plan) = sysmedic_fixes::fix_for_finding(&finding.id)
            .and_then(|fix_id| sysmedic_fixes::plan(fix_id, &report.snapshot))
        {
            let button = gtk::Button::with_label(strings.apply_fix);
            button.add_css_class("suggested-action");
            button.set_valign(gtk::Align::Center);
            let window = window.clone();
            let toasts = toasts.clone();
            let on_changed = on_changed.clone();
            button.connect_clicked(move |_| {
                confirm_and_apply(&window, lang, &plan, &toasts, on_changed.clone());
            });
            row.add_suffix(&button);
        }

        findings.append(&row);
    }
    root.append(&findings);

    // Skipped checks, if any.
    if !report.snapshot.collection_errors.is_empty() {
        root.append(&section_label(strings.skipped_checks));
        let skipped = boxed_list();
        for error in &report.snapshot.collection_errors {
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(error))
                .build();
            row.add_css_class("dim-label");
            skipped.append(&row);
        }
        root.append(&skipped);
    }

    root
}

/// Render an RFC3339 UTC timestamp in the user's local time and locale
/// (falling back to the raw string if it doesn't parse).
fn local_timestamp(rfc3339: &str) -> String {
    glib::DateTime::from_iso8601(rfc3339, None)
        .ok()
        .and_then(|dt| dt.to_local().ok())
        .and_then(|dt| dt.format("%x %X").ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| rfc3339.to_string())
}

fn boxed_list() -> gtk::ListBox {
    gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build()
}

fn section_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.add_css_class("heading");
    label.set_margin_top(6);
    label
}

fn detail_row(title: &str, subtitle: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(glib::markup_escape_text(title))
        .subtitle(glib::markup_escape_text(subtitle))
        .subtitle_lines(0)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_format_follows_the_extension() {
        use std::path::Path;
        assert_eq!(export_format_for(Path::new("r.md")), ExportFormat::Markdown);
        assert_eq!(
            export_format_for(Path::new("r.markdown")),
            ExportFormat::Markdown
        );
        assert_eq!(export_format_for(Path::new("r.json")), ExportFormat::Json);
        assert_eq!(export_format_for(Path::new("r.html")), ExportFormat::Html);
        // Unknown and missing extensions fall back to the dialog's default.
        assert_eq!(export_format_for(Path::new("r.txt")), ExportFormat::Html);
        assert_eq!(export_format_for(Path::new("report")), ExportFormat::Html);
        // Case is not significant on the extension.
        assert_eq!(export_format_for(Path::new("R.JSON")), ExportFormat::Json);
    }

    #[test]
    fn render_export_produces_the_selected_format() {
        use sysmedic_core::{Category, Finding, Severity, Snapshot};
        let report = HealthReport::build(
            Snapshot::default(),
            vec![Finding::new(
                "storage.disk_nearly_full",
                Category::Storage,
                Severity::Critical,
                "Filesystem / is 96% full",
                "Only 4 GiB free.",
            )],
        );
        let json = render_export(&report, ExportFormat::Json, Lang::En);
        assert!(serde_json_is_object(&json));
        let html = render_export(&report, ExportFormat::Html, Lang::Ar);
        assert!(html.contains("dir=\"rtl\""));
        let md = render_export(&report, ExportFormat::Markdown, Lang::En);
        assert!(md.starts_with('#'));
    }

    fn serde_json_is_object(s: &str) -> bool {
        s.trim_start().starts_with('{')
    }

    #[test]
    fn a_failed_helper_is_never_reported_as_success() {
        // The user must be told when pkexec was cancelled or the helper is
        // missing; a silent re-scan left them guessing whether it ran.
        assert_eq!(fix_outcome(true), FixOutcome::Applied);
        assert_eq!(fix_outcome(false), FixOutcome::Failed);
    }

    #[test]
    fn irreversible_fixes_get_the_destructive_style() {
        assert_eq!(
            response_appearance_for(false),
            adw::ResponseAppearance::Destructive
        );
        assert_eq!(
            response_appearance_for(true),
            adw::ResponseAppearance::Suggested
        );
    }

    #[test]
    fn local_timestamp_falls_back_to_the_raw_string() {
        assert_eq!(local_timestamp("not a timestamp"), "not a timestamp");
        // A valid RFC3339 stamp renders as something else (locale-dependent).
        assert_ne!(
            local_timestamp("2026-08-03T12:00:00Z"),
            "2026-08-03T12:00:00Z"
        );
    }
}

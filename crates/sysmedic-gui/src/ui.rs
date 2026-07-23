//! Widget construction. All strings and styling decisions come from the
//! `viewmodel` module; the checkup runs via `gio::spawn_blocking` so the
//! main loop stays responsive.

use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};
use sysmedic_core::fix::FixPlan;
use sysmedic_core::{Engine, HealthReport};
use sysmedic_knowledge::Lang;

use crate::viewmodel::{self, Strings};

const REPO_URL: &str = "https://github.com/abosalehg-ui/SysMedic";
const ISSUES_URL: &str = "https://github.com/abosalehg-ui/SysMedic/issues";

const DEFAULT_HELPER: &str = "/usr/libexec/sysmedic-fix-helper";

/// A "re-run the checkup" callback, shared so a finished fix can trigger it.
type RefreshFn = Rc<dyn Fn()>;

fn helper_path() -> String {
    std::env::var("SYSMEDIC_HELPER").unwrap_or_else(|_| DEFAULT_HELPER.to_string())
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
    Engine::new()
        .with_collectors(sysmedic_collectors::default_collectors())
        .with_diagnostics(sysmedic_diagnostics::default_diagnostics())
        .run()
}

pub fn build_window(app: &adw::Application) {
    let lang = Lang::from_locale(&std::env::var("LANG").unwrap_or_default());
    let strings = Strings::for_lang(lang);

    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.set_tooltip_text(Some(strings.run_checkup));
    let spinner = gtk::Spinner::new();

    let header = adw::HeaderBar::new();
    header.pack_start(&refresh);
    header.pack_end(&spinner);

    // Primary menu with an "About SysMedic" entry (author + repo live there).
    let menu = gio::Menu::new();
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

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&scrolled));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("SysMedic")
        .default_width(780)
        .default_height(860)
        .content(&toolbar_view)
        .build();

    // Wire the "app.about" action to show the About dialog.
    let about_action = gio::SimpleAction::new("about", None);
    about_action.connect_activate({
        let window = window.clone();
        move |_, _| show_about(&window, lang)
    });
    app.add_action(&about_action);

    // `run_checkup` needs to reference itself so a finished fix can trigger a
    // re-scan. A shared cell breaks the chicken-and-egg of the self-reference.
    let self_ref: Rc<RefCell<Option<RefreshFn>>> = Rc::new(RefCell::new(None));
    let run_checkup: RefreshFn = Rc::new({
        let clamp = clamp.clone();
        let spinner = spinner.clone();
        let refresh = refresh.clone();
        let window = window.clone();
        let self_ref = self_ref.clone();
        move || {
            spinner.start();
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
            let on_changed = self_ref.borrow().clone();
            glib::spawn_future_local(async move {
                let result = gtk::gio::spawn_blocking(run_engine).await;
                spinner.stop();
                refresh.set_sensitive(true);
                match result {
                    Ok(report) => {
                        let refresh_cb = on_changed.unwrap_or_else(|| Rc::new(|| {}));
                        clamp.set_child(Some(&report_view(&report, lang, &window, refresh_cb)));
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
    run_checkup();

    window.present();
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

/// Ask polkit (via pkexec) to run the helper for `fix_id`, then re-scan.
fn confirm_and_apply(
    window: &adw::ApplicationWindow,
    lang: Lang,
    plan: &FixPlan,
    on_changed: RefreshFn,
) {
    let strings = Strings::for_lang(lang);
    let reversibility = if plan.reversible {
        strings.reversible_yes
    } else {
        strings.reversible_no
    };
    let dialog = adw::AlertDialog::new(Some(strings.confirm_fix_title), None);
    dialog.set_body(&format!("{}\n\n{}", plan.preview(), reversibility));
    dialog.add_response("cancel", strings.cancel);
    dialog.add_response("apply", strings.apply);
    dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    let fix_id = plan.id.clone();
    let window = window.clone();
    dialog.connect_response(None, move |_, response| {
        if response != "apply" {
            return;
        }
        let fix_id = fix_id.clone();
        let on_changed = on_changed.clone();
        glib::spawn_future_local(async move {
            let helper = helper_path();
            let id = fix_id.clone();
            // pkexec prompts polkit; the helper does the privileged work.
            let _succeeded = gtk::gio::spawn_blocking(move || {
                Command::new("pkexec")
                    .arg(helper)
                    .arg("apply")
                    .arg(&id)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            })
            .await;
            on_changed(); // re-scan so the UI reflects the new state
        });
    });
    dialog.present(Some(&window));
}

fn report_view(
    report: &HealthReport,
    lang: Lang,
    window: &adw::ApplicationWindow,
    on_changed: RefreshFn,
) -> gtk::Box {
    let strings = Strings::for_lang(lang);
    let root = gtk::Box::new(gtk::Orientation::Vertical, 18);

    // Hero: the big score.
    let score = gtk::Label::new(Some(&format!("{}", report.score)));
    score.add_css_class("score-title");
    score.add_css_class(viewmodel::score_css(report.score));
    let grade = gtk::Label::new(Some(&format!("{} · {}/100", report.grade, report.score)));
    grade.add_css_class("title-2");
    let generated = gtk::Label::new(Some(&report.generated_at));
    generated.add_css_class("dim-label");
    generated.add_css_class("caption");
    let hero_title = gtk::Label::new(Some(strings.health_score));
    hero_title.add_css_class("dim-label");
    for w in [&hero_title, &score, &grade, &generated] {
        root.append(w);
    }

    // Category scores.
    root.append(&section_label(strings.categories));
    let categories = boxed_list();
    for row in viewmodel::category_rows(report) {
        let action_row = adw::ActionRow::builder().title(row.label).build();
        let bar = gtk::LevelBar::for_interval(0.0, 100.0);
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
            .title(glib::markup_escape_text(&finding.title))
            .subtitle(glib::markup_escape_text(&finding.summary))
            .build();
        let badge = gtk::Label::new(Some(&finding.severity.label().to_uppercase()));
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
            let on_changed = on_changed.clone();
            button.connect_clicked(move |_| {
                confirm_and_apply(&window, lang, &plan, on_changed.clone());
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

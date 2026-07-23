//! Widget construction. All strings and styling decisions come from the
//! `viewmodel` module; the checkup runs via `gio::spawn_blocking` so the
//! main loop stays responsive.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use sysmedic_core::{Engine, HealthReport};
use sysmedic_knowledge::Lang;

use crate::viewmodel::{self, Strings};

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

    let run_checkup: Rc<dyn Fn()> = Rc::new({
        let clamp = clamp.clone();
        let spinner = spinner.clone();
        let refresh = refresh.clone();
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
            glib::spawn_future_local(async move {
                let result = gtk::gio::spawn_blocking(run_engine).await;
                spinner.stop();
                refresh.set_sensitive(true);
                match result {
                    Ok(report) => clamp.set_child(Some(&report_view(&report, lang))),
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

    refresh.connect_clicked({
        let run_checkup = run_checkup.clone();
        move |_| run_checkup()
    });
    run_checkup();

    window.present();
}

fn report_view(report: &HealthReport, lang: Lang) -> gtk::Box {
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

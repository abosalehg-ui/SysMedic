//! SysMedic desktop application (GTK4 + libadwaita).
//!
//! MVVM-lite: `viewmodel` holds all presentation logic as pure, unit-tested
//! functions; `ui` builds widgets from view-model data and owns no logic of
//! its own. The checkup engine runs on a worker thread so the UI never
//! blocks.

mod disk;
mod ui;
mod viewmodel;

pub const APP_ID: &str = "io.github.abosalehg_ui.SysMedic";

fn main() -> gtk::glib::ExitCode {
    use gtk::prelude::*;

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| ui::load_css());
    app.connect_activate(ui::build_window);
    app.run()
}

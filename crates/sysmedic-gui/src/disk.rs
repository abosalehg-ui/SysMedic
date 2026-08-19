//! The Disk Usage page: a squarified treemap of the largest directories,
//! drawn with cairo, plus an accessible list of the same data. The scan runs
//! on a worker thread so the UI stays live, reports progress while it walks,
//! and can be cancelled.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use adw::prelude::*;
use gtk::glib;
use sysmedic_diskscan::{human_size, squarify, Node, Rect, ScanObserver};
use sysmedic_knowledge::Lang;

use crate::viewmodel::Strings;

/// How many top-level entries to show as treemap tiles.
const MAX_TILES: usize = 60;
/// How many entries the accessible list below the treemap shows.
const MAX_LIST_ROWS: usize = 8;

/// A small curated palette (GNOME palette shades) instead of arbitrary
/// hash-derived HSL: the old saturated candy colors clashed with the
/// otherwise restrained Adwaita look. Hashing picks a stable entry per label.
const PALETTE: [(f64, f64, f64); 8] = [
    (0.11, 0.44, 0.85), // blue4    #1c71d8
    (0.15, 0.64, 0.41), // green4   #26a269
    (0.90, 0.65, 0.04), // yellow4  #e5a50a
    (0.90, 0.38, 0.00), // orange4  #e66100
    (0.57, 0.25, 0.67), // purple3  #9141ac
    (0.60, 0.42, 0.27), // brown3   #986a44
    (0.37, 0.36, 0.39), // dark3    #5e5c64
    (0.38, 0.63, 0.92), // blue2    #62a0ea
];

/// Deterministic palette color per label (stable across redraws).
fn color_for(label: &str) -> (f64, f64, f64) {
    let mut hash: u32 = 2166136261;
    for b in label.bytes() {
        hash = (hash ^ b as u32).wrapping_mul(16777619);
    }
    PALETTE[(hash % PALETTE.len() as u32) as usize]
}

/// Black or white label text, picked by the tile's relative luminance so the
/// label passes contrast on light tiles (the old fixed white text failed on
/// yellow/light-blue).
fn text_color_for(bg: (f64, f64, f64)) -> (f64, f64, f64) {
    let luminance = 0.2126 * bg.0 + 0.7152 * bg.1 + 0.0722 * bg.2;
    if luminance > 0.55 {
        (0.10, 0.10, 0.12)
    } else {
        (1.0, 1.0, 1.0)
    }
}

/// Shared scan state: lets the UI thread cancel the worker and read progress.
#[derive(Default)]
struct ScanControl {
    cancelled: AtomicBool,
    visited: AtomicU64,
}

impl ScanObserver for ScanControl {
    fn should_continue(&self) -> bool {
        !self.cancelled.load(Ordering::Relaxed)
    }
    fn visited(&self, count: u64, _path: &std::path::Path) {
        self.visited.store(count, Ordering::Relaxed);
    }
}

/// Which tile the keyboard is on. `None` means nothing is selected yet.
type Selection = Rc<RefCell<Option<usize>>>;
/// Starts a scan of the given directory.
type ScanFn = Rc<dyn Fn(std::path::PathBuf)>;

pub fn disk_page(lang: Lang) -> gtk::Box {
    let strings = Strings::for_lang(lang);
    let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let heading = gtk::Label::new(Some(strings.disk_usage));
    heading.add_css_class("title-2");
    heading.set_xalign(0.0);
    let subtitle = gtk::Label::new(Some(strings.disk_scanning));
    subtitle.add_css_class("dim-label");
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);

    // Controls: choose a folder, and stop a running scan.
    let choose = gtk::Button::from_icon_name("folder-open-symbolic");
    choose.set_tooltip_text(Some(strings.disk_choose_folder));
    let cancel = gtk::Button::from_icon_name("process-stop-symbolic");
    cancel.set_tooltip_text(Some(strings.disk_cancel));
    cancel.add_css_class("destructive-action");
    cancel.set_visible(false);

    let header_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header_row.append(&heading);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    header_row.append(&spacer);
    header_row.append(&cancel);
    header_row.append(&choose);
    root.append(&header_row);
    root.append(&subtitle);

    // No meaningful percentage for an unbounded walk, so this pulses; the
    // entry count in the subtitle is what actually proves progress.
    let progress = gtk::ProgressBar::new();
    progress.set_visible(false);
    root.append(&progress);

    let tree: Rc<RefCell<Option<Node>>> = Rc::new(RefCell::new(None));
    let tiles: Rc<RefCell<Vec<sysmedic_diskscan::Tile>>> = Rc::new(RefCell::new(Vec::new()));
    let selection: Selection = Rc::new(RefCell::new(None));

    let area = gtk::DrawingArea::builder()
        .vexpand(true)
        .hexpand(true)
        .accessible_role(gtk::AccessibleRole::Img)
        .build();
    // The canvas itself is invisible to assistive tech; name it and point at
    // the equivalent list below.
    area.update_property(&[gtk::accessible::Property::Label(strings.treemap_a11y)]);
    area.set_tooltip_text(Some(strings.treemap_a11y));
    // Keyboard users could previously only reach the list; the tiles are now
    // focusable and walkable with the arrow keys, with the focused tile
    // outlined and its name announced.
    area.set_focusable(true);
    area.set_can_focus(true);

    // The treemap is drawn by hand with cairo, so GTK's automatic mirroring
    // does not reach it: in an Arabic window every other widget flipped while
    // the tiles stayed left-to-right, and the arrow keys walked against the
    // visual order. Both are mirrored explicitly below.
    let rtl = matches!(lang, Lang::Ar);

    area.set_draw_func({
        let tree = tree.clone();
        let tiles = tiles.clone();
        let selection = selection.clone();
        move |widget, cr, width, height| {
            let drawn = draw_treemap(
                cr,
                width,
                height,
                tree.borrow().as_ref(),
                *selection.borrow(),
                widget.has_focus(),
                rtl,
            );
            *tiles.borrow_mut() = drawn;
        }
    });
    root.append(&area);

    // Arrow keys move the selection; Home/End jump to largest/smallest.
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed({
        let tiles = tiles.clone();
        let selection = selection.clone();
        let area_ref = area.clone();
        let subtitle_ref = subtitle.clone();
        move |_, key, _, _| {
            let count = tiles.borrow().len();
            if count == 0 {
                return glib::Propagation::Proceed;
            }
            let current = selection.borrow().unwrap_or(0);
            let next = match key {
                // Down/Up always advance and retreat; the horizontal pair
                // follows the visual order, which is mirrored in RTL.
                gtk::gdk::Key::Down => Some((current + 1) % count),
                gtk::gdk::Key::Up => Some((current + count - 1) % count),
                gtk::gdk::Key::Right | gtk::gdk::Key::Left => {
                    let forward = (key == gtk::gdk::Key::Right) != rtl;
                    Some(if forward {
                        (current + 1) % count
                    } else {
                        (current + count - 1) % count
                    })
                }
                gtk::gdk::Key::Home => Some(0),
                gtk::gdk::Key::End => Some(count - 1),
                _ => None,
            };
            match next {
                Some(i) => {
                    *selection.borrow_mut() = Some(i);
                    let announcement = tiles
                        .borrow()
                        .get(i)
                        .map(|t| format!("{} — {}", t.label, human_size(t.size)));
                    if let Some(text) = announcement {
                        // The drawn label is invisible to a screen reader, so
                        // restate it as the widget's accessible name.
                        area_ref.update_property(&[gtk::accessible::Property::Label(&text)]);
                        subtitle_ref.set_text(&text);
                    }
                    area_ref.queue_draw();
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            }
        }
    });
    area.add_controller(keys);

    let focus = gtk::EventControllerFocus::new();
    focus.connect_enter({
        let area = area.clone();
        let selection = selection.clone();
        move |_| {
            if selection.borrow().is_none() {
                *selection.borrow_mut() = Some(0);
            }
            area.queue_draw();
        }
    });
    focus.connect_leave({
        let area = area.clone();
        move |_| area.queue_draw()
    });
    area.add_controller(focus);

    // The same data as rows — readable by screen readers and keyboard users.
    let list_heading = gtk::Label::new(Some(strings.disk_largest));
    list_heading.add_css_class("heading");
    list_heading.set_xalign(0.0);
    list_heading.set_visible(false);
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .visible(false)
        .build();
    root.append(&list_heading);
    root.append(&list);

    let start_scan: ScanFn = Rc::new({
        let tree = tree.clone();
        let area = area.clone();
        let subtitle = subtitle.clone();
        let progress = progress.clone();
        let cancel = cancel.clone();
        let list = list.clone();
        let list_heading = list_heading.clone();
        let selection = selection.clone();
        move |path: std::path::PathBuf| {
            let control = Arc::new(ScanControl::default());
            cancel.set_visible(true);
            progress.set_visible(true);
            subtitle.set_text(strings.disk_scanning);
            list.set_visible(false);
            list_heading.set_visible(false);
            *selection.borrow_mut() = None;

            let cancel_handler = cancel.connect_clicked({
                let control = control.clone();
                move |_| control.cancelled.store(true, Ordering::Relaxed)
            });

            // Pulse while the walk proceeds and show the running entry count:
            // an unbounded walk has no percentage, but "184320 entries" proves
            // the scan is alive rather than wedged.
            let ticker = glib::timeout_add_local(std::time::Duration::from_millis(150), {
                let control = control.clone();
                let progress = progress.clone();
                let subtitle = subtitle.clone();
                move || {
                    if !progress.is_visible() {
                        return glib::ControlFlow::Break;
                    }
                    progress.pulse();
                    let n = control.visited.load(Ordering::Relaxed);
                    if n > 0 {
                        subtitle.set_text(&format!("{} {n}", strings.disk_scanning));
                    }
                    glib::ControlFlow::Continue
                }
            });

            glib::spawn_future_local({
                let tree = tree.clone();
                let area = area.clone();
                let subtitle = subtitle.clone();
                let progress = progress.clone();
                let cancel = cancel.clone();
                let list = list.clone();
                let list_heading = list_heading.clone();
                let control = control.clone();
                let shown = path.clone();
                async move {
                    let scanned = gtk::gio::spawn_blocking({
                        let control = control.clone();
                        let path = path.clone();
                        move || sysmedic_diskscan::scan_observed(&path, 2, control.as_ref())
                    })
                    .await;

                    ticker.remove();
                    progress.set_visible(false);
                    cancel.set_visible(false);
                    cancel.disconnect(cancel_handler);

                    let was_cancelled = control.cancelled.load(Ordering::Relaxed);
                    match scanned {
                        Ok(node) if node.size == 0 || node.children.is_empty() => {
                            subtitle.set_text(if was_cancelled {
                                strings.disk_cancelled
                            } else {
                                strings.disk_empty
                            });
                        }
                        Ok(node) => {
                            let prefix = if was_cancelled {
                                strings.disk_partial
                            } else {
                                ""
                            };
                            subtitle.set_text(&format!(
                                "{prefix}{} — {}",
                                shown.display(),
                                human_size(node.size)
                            ));
                            while let Some(row) = list.first_child() {
                                list.remove(&row);
                            }
                            for child in node.children.iter().take(MAX_LIST_ROWS) {
                                let title = if child.is_dir {
                                    format!("{}/", child.name)
                                } else {
                                    child.name.clone()
                                };
                                let row = adw::ActionRow::builder()
                                    .title(glib::markup_escape_text(&title))
                                    .build();
                                let size = gtk::Label::new(Some(&human_size(child.size)));
                                size.add_css_class("numeric");
                                size.add_css_class("dim-label");
                                row.add_suffix(&size);
                                list.append(&row);
                            }
                            list_heading.set_visible(true);
                            list.set_visible(true);
                            *tree.borrow_mut() = Some(node);
                            area.queue_draw();
                        }
                        Err(_) => subtitle.set_text(strings.disk_scan_failed),
                    }
                }
            });
        }
    });

    // Let the user scan somewhere other than $HOME — the CLI has taken a path
    // argument since day one, so the GUI hard-coding the home folder was a
    // gap rather than a decision.
    choose.connect_clicked({
        let start_scan = start_scan.clone();
        let root_widget = root.clone();
        move |_| {
            let dialog = gtk::FileDialog::builder()
                .title(strings.disk_choose_folder)
                .build();
            let window = root_widget
                .root()
                .and_then(|r| r.downcast::<gtk::Window>().ok());
            let start_scan = start_scan.clone();
            dialog.select_folder(
                window.as_ref(),
                None::<&gtk::gio::Cancellable>,
                move |result| {
                    // A dismissed dialog is a cancellation, not an error.
                    if let Some(path) = result.ok().and_then(|f| f.path()) {
                        start_scan(path);
                    }
                },
            );
        }
    });

    // Scan lazily, the first time this page is actually shown.
    //
    // The page is built with the window, so kicking the scan off here walked
    // the whole of `$HOME` on every launch — minutes of I/O and battery on a
    // large home directory — even for a user who only ever opens the Overview
    // tab. On a machine slow enough to need SysMedic, the diagnostic tool was
    // itself the load.
    let scanned_once = Rc::new(std::cell::Cell::new(false));
    root.connect_map({
        let start_scan = start_scan.clone();
        let scanned_once = scanned_once.clone();
        let subtitle = subtitle.clone();
        move |_| {
            if scanned_once.replace(true) {
                return;
            }
            match std::env::var("HOME")
                .ok()
                .filter(|h| !h.is_empty())
                .map(std::path::PathBuf::from)
            {
                Some(path) => start_scan(path),
                // Silently scanning `/` instead would be a minutes-long walk
                // the user never asked for; invite them to pick a folder.
                None => subtitle.set_text(strings.disk_pick_a_folder),
            }
        }
    });

    root
}

/// Draw the treemap and return the tiles, so the keyboard handler can address
/// them by index and the focus ring lands on the right rectangle.
fn draw_treemap(
    cr: &gtk::cairo::Context,
    width: i32,
    height: i32,
    tree: Option<&Node>,
    selected: Option<usize>,
    focused: bool,
    rtl: bool,
) -> Vec<sysmedic_diskscan::Tile> {
    let (w, h) = (width as f64, height as f64);
    let Some(tree) = tree else {
        return Vec::new();
    };
    let items: Vec<(String, u64)> = tree
        .children
        .iter()
        .take(MAX_TILES)
        .map(|c| (c.name.clone(), c.size))
        .collect();
    let mut tiles = squarify(
        &items,
        Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        },
    );
    if rtl {
        for tile in &mut tiles {
            tile.rect.x = mirror_x(tile.rect.x, tile.rect.w, w);
        }
    }

    for (i, tile) in tiles.iter().enumerate() {
        let (r, g, b) = color_for(&tile.label);
        cr.rectangle(tile.rect.x, tile.rect.y, tile.rect.w, tile.rect.h);
        cr.set_source_rgb(r, g, b);
        let _ = cr.fill_preserve();
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.35);
        cr.set_line_width(1.0);
        let _ = cr.stroke();

        // Label the tile if it is large enough to read.
        if tile.rect.w > 64.0 && tile.rect.h > 30.0 {
            let (tr, tg, tb) = text_color_for((r, g, b));
            cr.set_source_rgb(tr, tg, tb);
            cr.select_font_face(
                "sans-serif",
                gtk::cairo::FontSlant::Normal,
                gtk::cairo::FontWeight::Bold,
            );
            cr.set_font_size(12.0);
            cr.move_to(tile.rect.x + 6.0, tile.rect.y + 18.0);
            let _ = cr.show_text(&tile.label);
            cr.set_font_size(10.0);
            cr.move_to(tile.rect.x + 6.0, tile.rect.y + 32.0);
            let _ = cr.show_text(&human_size(tile.size));
        }

        // A visible focus ring — keyboard focus that cannot be seen is not
        // keyboard support. Light stroke under a dark one so it reads against
        // every palette entry.
        if focused && selected == Some(i) {
            for (width, (cr_r, cr_g, cr_b)) in [(3.0, (1.0, 1.0, 1.0)), (1.0, (0.05, 0.05, 0.07))] {
                cr.rectangle(
                    tile.rect.x + 2.0,
                    tile.rect.y + 2.0,
                    (tile.rect.w - 4.0).max(0.0),
                    (tile.rect.h - 4.0).max(0.0),
                );
                cr.set_source_rgb(cr_r, cr_g, cr_b);
                cr.set_line_width(width);
                let _ = cr.stroke();
            }
        }
    }
    tiles
}

/// Flip a tile's x coordinate for a right-to-left layout: the largest entry
/// belongs in the top-*right* corner when the rest of the window reads that
/// way. Pure, so the mirroring is unit-tested without a drawing context.
fn mirror_x(x: f64, tile_width: f64, canvas_width: f64) -> f64 {
    canvas_width - x - tile_width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_size_formats() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(2048), "2.0 KiB");
    }

    #[test]
    fn colors_are_stable_and_in_range() {
        let a = color_for("Documents");
        let b = color_for("Documents");
        assert_eq!(a, b);
        for v in [a.0, a.1, a.2] {
            assert!((0.0..=1.0).contains(&v));
        }
    }

    #[test]
    fn label_text_contrasts_with_tile() {
        // Light tiles get dark text, dark tiles get light text.
        assert_eq!(text_color_for((0.90, 0.65, 0.04)).0, 0.10); // yellow4
        assert_eq!(text_color_for((0.11, 0.44, 0.85)).0, 1.0); // blue4
    }

    #[test]
    fn every_palette_entry_gets_readable_label_text() {
        // The luminance switch must yield a genuinely contrasting pair for
        // every colour actually drawn, not just the two spot-checked above.
        let lum = |c: (f64, f64, f64)| 0.2126 * c.0 + 0.7152 * c.1 + 0.0722 * c.2;
        for bg in PALETTE {
            let fg = text_color_for(bg);
            assert!(
                (lum(fg) - lum(bg)).abs() > 0.35,
                "label on {bg:?} is too close in luminance to its tile"
            );
        }
    }

    #[test]
    fn rtl_mirrors_tiles_across_the_canvas() {
        // A tile hugging the left edge in LTR must hug the right edge in RTL.
        assert_eq!(mirror_x(0.0, 100.0, 800.0), 700.0);
        assert_eq!(mirror_x(700.0, 100.0, 800.0), 0.0);
        // A full-width tile is unchanged, and mirroring twice is the identity.
        assert_eq!(mirror_x(0.0, 800.0, 800.0), 0.0);
        let (x, w, canvas) = (120.0, 60.0, 800.0);
        assert_eq!(mirror_x(mirror_x(x, w, canvas), w, canvas), x);
    }

    #[test]
    fn scan_control_cancels_and_records_progress() {
        let c = ScanControl::default();
        assert!(c.should_continue());
        c.visited(4096, std::path::Path::new("/home/u"));
        assert_eq!(c.visited.load(Ordering::Relaxed), 4096);
        c.cancelled.store(true, Ordering::Relaxed);
        assert!(!c.should_continue());
    }
}

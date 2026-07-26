//! The Disk Usage page: a squarified treemap of the largest directories,
//! drawn with cairo, plus an accessible list of the same data. The scan runs
//! on a worker thread so the UI stays live.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use sysmedic_diskscan::{human_size, squarify, Node, Rect};
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
    root.append(&heading);
    root.append(&subtitle);

    let tree: Rc<RefCell<Option<Node>>> = Rc::new(RefCell::new(None));
    let area = gtk::DrawingArea::builder()
        .vexpand(true)
        .hexpand(true)
        .accessible_role(gtk::AccessibleRole::Img)
        .build();
    // The canvas itself is invisible to assistive tech; name it and point at
    // the equivalent list below.
    area.update_property(&[gtk::accessible::Property::Label(strings.treemap_a11y)]);
    area.set_tooltip_text(Some(strings.treemap_a11y));

    area.set_draw_func({
        let tree = tree.clone();
        move |_, cr, width, height| draw_treemap(cr, width, height, tree.borrow().as_ref())
    });
    root.append(&area);

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

    // Scan the home directory in the background, then draw.
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    glib::spawn_future_local({
        let tree = tree.clone();
        let area = area.clone();
        let subtitle = subtitle.clone();
        let home = home.clone();
        async move {
            let scan_path = home.clone();
            let scanned =
                gtk::gio::spawn_blocking(move || sysmedic_diskscan::scan(&scan_path, 2)).await;
            match scanned {
                // An empty result usually means the folder was unreadable or
                // genuinely empty — either way, say so instead of leaving the
                // "Scanning…" subtitle up forever over a blank canvas.
                Ok(node) if node.size == 0 || node.children.is_empty() => {
                    subtitle.set_text(strings.disk_empty);
                }
                Ok(node) => {
                    subtitle.set_text(&format!("{} — {}", home, human_size(node.size)));
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

    root
}

fn draw_treemap(cr: &gtk::cairo::Context, width: i32, height: i32, tree: Option<&Node>) {
    let (w, h) = (width as f64, height as f64);
    let Some(tree) = tree else {
        return;
    };
    let items: Vec<(String, u64)> = tree
        .children
        .iter()
        .take(MAX_TILES)
        .map(|c| (c.name.clone(), c.size))
        .collect();
    let tiles = squarify(
        &items,
        Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        },
    );

    for tile in &tiles {
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
    }
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
}

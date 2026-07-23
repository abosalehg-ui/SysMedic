//! The Disk Usage page: a squarified treemap of the largest directories,
//! drawn with cairo. The scan runs on a worker thread so the UI stays live.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use sysmedic_diskscan::{squarify, Node, Rect};

/// How many top-level entries to show as treemap tiles.
const MAX_TILES: usize = 60;

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// Deterministic pleasant-ish color per label (stable across redraws).
fn color_for(label: &str) -> (f64, f64, f64) {
    let mut hash: u32 = 2166136261;
    for b in label.bytes() {
        hash = (hash ^ b as u32).wrapping_mul(16777619);
    }
    let hue = (hash % 360) as f64;
    hsl_to_rgb(hue, 0.55, 0.55)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (r + m, g + m, b + m)
}

pub fn disk_page() -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let heading = gtk::Label::new(Some("Disk usage"));
    heading.add_css_class("title-2");
    heading.set_xalign(0.0);
    let subtitle = gtk::Label::new(Some("Scanning your home folder…"));
    subtitle.add_css_class("dim-label");
    subtitle.set_xalign(0.0);
    root.append(&heading);
    root.append(&subtitle);

    let tree: Rc<RefCell<Option<Node>>> = Rc::new(RefCell::new(None));
    let area = gtk::DrawingArea::builder()
        .vexpand(true)
        .hexpand(true)
        .build();

    area.set_draw_func({
        let tree = tree.clone();
        move |_, cr, width, height| draw_treemap(cr, width, height, tree.borrow().as_ref())
    });
    root.append(&area);

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
            if let Ok(node) = scanned {
                subtitle.set_text(&format!("{} — {}", home, human_size(node.size)));
                *tree.borrow_mut() = Some(node);
                area.queue_draw();
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
            cr.set_source_rgb(1.0, 1.0, 1.0);
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
}

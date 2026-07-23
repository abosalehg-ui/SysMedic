//! Squarified treemap layout (Bruls, Huizing & van Wijk, 2000).
//!
//! Turns a set of weighted items into rectangles that tile a bounding box,
//! favouring square-ish tiles so labels stay readable. Pure math — no GTK —
//! so it is unit-tested directly.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn area(&self) -> f64 {
        self.w * self.h
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Tile {
    pub label: String,
    pub size: u64,
    pub rect: Rect,
}

/// Lay `items` (label, weight) out inside `bounds`. Items should be sorted
/// largest-first for the nicest result; zero-weight items and degenerate
/// bounds yield no tiles.
pub fn squarify(items: &[(String, u64)], bounds: Rect) -> Vec<Tile> {
    let total: u64 = items.iter().map(|(_, w)| *w).sum();
    if total == 0 || bounds.w <= 0.0 || bounds.h <= 0.0 {
        return Vec::new();
    }
    let scale = bounds.area() / total as f64;
    // (label, size, scaled area)
    let scaled: Vec<(String, u64, f64)> = items
        .iter()
        .filter(|(_, w)| *w > 0)
        .map(|(l, w)| (l.clone(), *w, *w as f64 * scale))
        .collect();

    let mut tiles = Vec::with_capacity(scaled.len());
    let mut rect = bounds;
    let mut row: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < scaled.len() {
        let side = rect.w.min(rect.h);
        let row_areas: Vec<f64> = row.iter().map(|&j| scaled[j].2).collect();
        let with_next: Vec<f64> = row_areas
            .iter()
            .copied()
            .chain(std::iter::once(scaled[i].2))
            .collect();
        if row.is_empty() || worst_ratio(&with_next, side) <= worst_ratio(&row_areas, side) {
            row.push(i);
            i += 1;
        } else {
            layout_row(&mut tiles, &row, &scaled, &mut rect);
            row.clear();
        }
    }
    if !row.is_empty() {
        layout_row(&mut tiles, &row, &scaled, &mut rect);
    }
    tiles
}

/// Worst (largest) aspect ratio in a row laid along a side of length `side`.
fn worst_ratio(areas: &[f64], side: f64) -> f64 {
    if areas.is_empty() || side <= 0.0 {
        return f64::INFINITY;
    }
    let sum: f64 = areas.iter().sum();
    let max = areas.iter().copied().fold(f64::MIN, f64::max);
    let min = areas.iter().copied().fold(f64::MAX, f64::min);
    let s2 = sum * sum;
    let side2 = side * side;
    (side2 * max / s2).max(s2 / (side2 * min))
}

fn layout_row(
    tiles: &mut Vec<Tile>,
    row: &[usize],
    scaled: &[(String, u64, f64)],
    rect: &mut Rect,
) {
    let row_area: f64 = row.iter().map(|&j| scaled[j].2).sum();
    if rect.w >= rect.h {
        // Lay the row as a column on the left edge.
        let w = row_area / rect.h;
        let mut y = rect.y;
        for &j in row {
            let h = scaled[j].2 / w;
            tiles.push(Tile {
                label: scaled[j].0.clone(),
                size: scaled[j].1,
                rect: Rect { x: rect.x, y, w, h },
            });
            y += h;
        }
        rect.x += w;
        rect.w -= w;
    } else {
        // Lay the row along the top edge.
        let h = row_area / rect.w;
        let mut x = rect.x;
        for &j in row {
            let w = scaled[j].2 / h;
            tiles.push(Tile {
                label: scaled[j].0.clone(),
                size: scaled[j].1,
                rect: Rect { x, y: rect.y, w, h },
            });
            x += w;
        }
        rect.y += h;
        rect.h -= h;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        }
    }

    #[test]
    fn tiles_are_area_proportional_and_fill_bounds() {
        let items = vec![
            ("a".to_string(), 50),
            ("b".to_string(), 30),
            ("c".to_string(), 20),
        ];
        let tiles = squarify(&items, bounds());
        assert_eq!(tiles.len(), 3);

        // Every tile's area matches its weight share of the bounds.
        let total_area: f64 = tiles.iter().map(|t| t.rect.area()).sum();
        assert!((total_area - bounds().area()).abs() < 1.0);
        for tile in &tiles {
            let expected = tile.size as f64 / 100.0 * bounds().area();
            assert!(
                (tile.rect.area() - expected).abs() < 1.0,
                "{} area {} vs expected {expected}",
                tile.label,
                tile.rect.area()
            );
        }
    }

    #[test]
    fn tiles_stay_within_bounds() {
        let items: Vec<(String, u64)> = (0..12).map(|i| (format!("d{i}"), 12 - i)).collect();
        let tiles = squarify(&items, bounds());
        for t in &tiles {
            assert!(t.rect.x >= -0.001 && t.rect.y >= -0.001);
            assert!(t.rect.x + t.rect.w <= 100.001);
            assert!(t.rect.y + t.rect.h <= 100.001);
        }
    }

    #[test]
    fn empty_and_degenerate_inputs_are_safe() {
        assert!(squarify(&[], bounds()).is_empty());
        assert!(squarify(&[("z".into(), 0)], bounds()).is_empty());
        assert!(squarify(
            &[("a".into(), 1)],
            Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 10.0
            }
        )
        .is_empty());
    }
}

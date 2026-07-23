//! Disk-usage scanning and treemap layout for the disk analyzer.
//!
//! [`scan`] walks a directory into a size [`Node`] tree (I/O, but small and
//! self-contained). [`squarify`] turns a set of sibling sizes into rectangles
//! for the GUI treemap — a pure function, unit-tested, with no GTK/cairo
//! dependency so the layout math is verified in isolation.

pub mod treemap;

use std::fs;
use std::path::Path;

use serde::Serialize;

pub use treemap::{squarify, Rect, Tile};

/// A file or directory with its total size and (for directories) children.
#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    pub children: Vec<Node>,
}

/// Scan `root` into a size tree, descending at most `max_depth` levels.
/// Directory sizes are the recursive sum of their contents even when the
/// walk stops descending (so totals stay accurate); children beyond the
/// depth limit are simply not listed. Unreadable entries are skipped.
pub fn scan(root: impl AsRef<Path>, max_depth: u32) -> Node {
    let root = root.as_ref();
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    scan_inner(root, &name, max_depth)
}

fn scan_inner(path: &Path, name: &str, depth_left: u32) -> Node {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return Node {
            name: name.to_string(),
            size: 0,
            is_dir: false,
            children: Vec::new(),
        };
    };

    if !meta.is_dir() {
        return Node {
            name: name.to_string(),
            size: meta.len(),
            is_dir: false,
            children: Vec::new(),
        };
    }

    let mut children = Vec::new();
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let child_name = entry.file_name().to_string_lossy().into_owned();
            let child = scan_inner(&entry.path(), &child_name, depth_left.saturating_sub(1));
            total += child.size;
            if depth_left > 0 {
                children.push(child);
            }
        }
    }
    children.sort_by(|a, b| b.size.cmp(&a.size));
    Node {
        name: name.to_string(),
        size: total,
        is_dir: true,
        children,
    }
}

/// The `limit` largest direct children of `node`, for a "top directories"
/// listing (any remainder is not included).
pub fn largest_children(node: &Node, limit: usize) -> Vec<&Node> {
    node.children.iter().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_file(path: &Path, bytes: usize) {
        let mut f = fs::File::create(path).unwrap();
        f.write_all(&vec![0u8; bytes]).unwrap();
    }

    #[test]
    fn scans_sizes_and_sorts_children() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join("big")).unwrap();
        fs::create_dir(root.join("small")).unwrap();
        write_file(&root.join("big/a.bin"), 5000);
        write_file(&root.join("small/b.bin"), 100);

        let tree = scan(root, 3);
        assert!(tree.is_dir);
        assert_eq!(tree.size, 5100);
        // Children are sorted largest-first.
        assert_eq!(tree.children[0].name, "big");
        assert_eq!(tree.children[0].size, 5000);
    }

    #[test]
    fn depth_limit_keeps_totals_but_drops_deep_children() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("a/b")).unwrap();
        write_file(&root.join("a/b/deep.bin"), 2000);

        let tree = scan(root, 1);
        // Total is still correct...
        assert_eq!(tree.size, 2000);
        // ...but we only kept one level of children.
        assert_eq!(tree.children[0].name, "a");
        assert!(tree.children[0].children.is_empty());
    }
}

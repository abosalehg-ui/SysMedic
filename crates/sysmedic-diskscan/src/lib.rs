//! Disk-usage scanning and treemap layout for the disk analyzer.
//!
//! [`scan`] walks a directory into a size [`Node`] tree (I/O, but small and
//! self-contained). [`squarify`] turns a set of sibling sizes into rectangles
//! for the GUI treemap — a pure function, unit-tested, with no GTK/cairo
//! dependency so the layout math is verified in isolation.

pub mod treemap;

use std::fs;
use std::os::unix::fs::MetadataExt as _;
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

/// Cooperative cancellation and progress reporting for a scan.
///
/// A home directory can hold hundreds of thousands of files, and the walk
/// descends fully even when the depth limit caps what is *kept* (the totals
/// have to be right). Without this the UI sat on "Scanning…" with no progress
/// and no way out — the one long operation in the app was also the only one
/// the user could not abandon.
pub trait ScanObserver: Sync {
    /// Called periodically. Return `false` to abort the walk; the partial tree
    /// built so far is returned.
    fn should_continue(&self) -> bool {
        true
    }
    /// Called as entries are visited, with the running count and the entry.
    fn visited(&self, _count: u64, _path: &Path) {}
}

/// An observer that never cancels and ignores progress.
pub struct NoopObserver;
impl ScanObserver for NoopObserver {}

/// Scan `root` into a size tree, descending at most `max_depth` levels.
/// Directory sizes are the recursive sum of their contents even when the
/// walk stops descending (so totals stay accurate); children beyond the
/// depth limit are simply not listed. Unreadable entries are skipped.
pub fn scan(root: impl AsRef<Path>, max_depth: u32) -> Node {
    scan_observed(root, max_depth, &NoopObserver)
}

/// [`scan`] that reports progress and can be cancelled through `observer`.
///
/// On cancellation the tree built so far is returned rather than an error:
/// a partial answer ("your Downloads folder is already 40 GiB") is more useful
/// than nothing, and the caller knows it asked to stop.
pub fn scan_observed(root: impl AsRef<Path>, max_depth: u32, observer: &dyn ScanObserver) -> Node {
    let root = root.as_ref();
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    // The device id of the starting filesystem. The walk stays on it, like
    // `du -x`, so a scan of `/` doesn't wander into `/proc` (where `/proc/kcore`
    // alone reports ~128 TiB), `/sys`, `/dev`, or other mounted disks.
    let root_dev = fs::metadata(root).map(|m| m.dev()).ok();
    let mut visited = 0u64;
    scan_inner(root, &name, max_depth, root_dev, observer, &mut visited)
}

fn scan_inner(
    path: &Path,
    name: &str,
    depth_left: u32,
    root_dev: Option<u64>,
    observer: &dyn ScanObserver,
    visited: &mut u64,
) -> Node {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return Node {
            name: name.to_string(),
            size: 0,
            is_dir: false,
            children: Vec::new(),
        };
    };

    // A different device id means a separate mount — skip it and its subtree.
    if let Some(dev) = root_dev {
        if meta.dev() != dev {
            return Node {
                name: name.to_string(),
                size: 0,
                is_dir: meta.is_dir(),
                children: Vec::new(),
            };
        }
    }

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
            // Check cancellation every 512 entries: often enough to feel
            // responsive, rare enough not to make the atomic load the
            // bottleneck of the walk.
            *visited += 1;
            if (*visited).is_multiple_of(512) {
                observer.visited(*visited, path);
                if !observer.should_continue() {
                    break;
                }
            }
            let child_name = entry.file_name().to_string_lossy().into_owned();
            let child = scan_inner(
                &entry.path(),
                &child_name,
                depth_left.saturating_sub(1),
                root_dev,
                observer,
                visited,
            );
            // Saturating: a pathological tree (hard links counted repeatedly,
            // a sparse file reporting a huge apparent size) must not panic the
            // scan in a debug build.
            total = total.saturating_add(child.size);
            if depth_left > 0 {
                children.push(child);
            }
        }
    }
    children.sort_by_key(|c| std::cmp::Reverse(c.size));
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

/// Human-readable byte size, e.g. `512 B`, `1.5 KiB`, `5.0 GiB`. Shared by the
/// CLI and GUI so the two can't drift.
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
    fn a_cancelling_observer_stops_the_walk_and_returns_a_partial_tree() {
        use std::sync::atomic::{AtomicU64, Ordering};
        struct CancelAfter(AtomicU64);
        impl ScanObserver for CancelAfter {
            fn should_continue(&self) -> bool {
                false // cancel at the first checkpoint
            }
            fn visited(&self, count: u64, _: &Path) {
                self.0.store(count, Ordering::Relaxed);
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Enough entries to cross the 512-entry cancellation checkpoint.
        for i in 0..1200 {
            write_file(&root.join(format!("f{i}.bin")), 10);
        }
        let observer = CancelAfter(AtomicU64::new(0));
        let tree = scan_observed(root, 2, &observer);

        assert!(
            observer.0.load(Ordering::Relaxed) >= 512,
            "never checked in"
        );
        // Stopped early, so it did not total all 1200 files.
        assert!(
            tree.size < 12_000,
            "walk did not stop on cancel (size {})",
            tree.size
        );
    }

    #[test]
    fn an_observer_that_never_cancels_matches_a_plain_scan() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join("a")).unwrap();
        write_file(&root.join("a/x.bin"), 4096);
        assert_eq!(
            scan(root, 3).size,
            scan_observed(root, 3, &NoopObserver).size
        );
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

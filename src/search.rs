//! Simple file search index
//!
//! Uses `walkdir` to collect file paths and `fst` to build a compact, fast
//! prefix-searchable set.

use anyhow::Result;
use fst::{Automaton, IntoStreamer, Set, Streamer};
use fst::automaton::Str;
use walkdir::WalkDir;
use std::path::PathBuf;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};

static GLOBAL_INDEX: OnceCell<Arc<RwLock<Option<SearchIndex>>>> = OnceCell::new();
static SCANNED_COUNT: AtomicUsize = AtomicUsize::new(0);
static IS_BUILDING: AtomicBool = AtomicBool::new(false);

/// Set the global index
pub fn set_global_index(idx: Arc<RwLock<Option<SearchIndex>>>) {
    let _ = GLOBAL_INDEX.set(idx);
}

/// Get the global index handle
pub fn global_index() -> Option<Arc<RwLock<Option<SearchIndex>>>> {
    GLOBAL_INDEX.get().cloned()
}

/// Check if index is ready
pub fn is_index_ready() -> bool {
    if let Some(g) = global_index() {
        g.read().is_some()
    } else {
        false
    }
}

/// Check if we're in the building phase (sorting/FST creation)
pub fn is_building() -> bool {
    IS_BUILDING.load(Ordering::Relaxed)
}

/// Get current scanned file count (for progress display)
pub fn scanned_count() -> usize {
    SCANNED_COUNT.load(Ordering::Relaxed)
}

/// A simple in-memory search index built from filenames -> full paths.
pub struct SearchIndex {
    set: Set<Vec<u8>>,
    count: usize,
}

impl SearchIndex {
    /// Build an index from the provided roots (walks recursively).
    pub fn build(roots: &[PathBuf]) -> Result<Self> {
        Self::build_with_excludes(roots, &[])
    }

    /// Build an index with exclusion patterns
    pub fn build_with_excludes(roots: &[PathBuf], exclude_patterns: &[String]) -> Result<Self> {
        let mut keys: Vec<String> = Vec::new();

        log::info!("Starting search index build...");
        SCANNED_COUNT.store(0, Ordering::Relaxed);
        IS_BUILDING.store(false, Ordering::Relaxed);

        // Compile glob patterns for exclusion
        let exclude_globs: Vec<glob::Pattern> = exclude_patterns
            .iter()
            .filter_map(|pattern| glob::Pattern::new(pattern).ok())
            .collect();

        for root in roots {
            log::info!("Indexing directory: {}", root.display());
            let walker = WalkDir::new(root).follow_links(false).into_iter();

            for entry in walker.filter_map(|e| e.ok()) {
                let path_str = entry.path().to_string_lossy();

                // Check exclusions
                if exclude_globs.iter().any(|p| p.matches(&path_str)) {
                    continue;
                }

                if entry.file_type().is_file() {
                    SCANNED_COUNT.fetch_add(1, Ordering::Relaxed);
                    let filename = entry.file_name().to_string_lossy().to_lowercase();
                    let full = entry.path().to_string_lossy().to_string();
                    keys.push(format!("{}\0{}", filename, full));
                }
            }
        }

        log::info!("Collected {} files, sorting...", keys.len());
        IS_BUILDING.store(true, Ordering::Relaxed);
        keys.sort();

        log::info!("Building FST index...");
        let set = Set::from_iter(keys.iter())?;
        let count = set.len();

        IS_BUILDING.store(false, Ordering::Relaxed);
        log::info!("Search index ready with {} files", count);

        Ok(Self { set, count })
    }

    /// Return the number of indexed entries
    pub fn count(&self) -> usize {
        self.count
    }

    /// Search for filenames that start with `prefix` (case-insensitive)
    pub fn search_prefix(&self, prefix: &str, limit: usize) -> Vec<String> {
        let q = prefix.to_lowercase();
        let matcher = Str::new(&q).starts_with();
        let mut stream = self.set.search(&matcher).into_stream();

        let mut res = Vec::new();
        while let Some(key) = stream.next() {
            if res.len() >= limit {
                break;
            }
            if let Ok(s) = std::str::from_utf8(key) {
                if let Some(pos) = s.find('\0') {
                    res.push(s[pos + 1..].to_string());
                }
            }
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn build_and_search() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("Hello.txt")).unwrap();
        File::create(dir.path().join("hello_world.md")).unwrap();
        File::create(dir.path().join("Other.dat")).unwrap();

        let idx = SearchIndex::build(&[dir.path().to_path_buf()]).unwrap();
        assert!(idx.count() >= 3);

        let results = idx.search_prefix("hel", 10);
        assert!(results.iter().any(|p| p.ends_with("Hello.txt")));
        assert!(results.iter().any(|p| p.ends_with("hello_world.md")));
    }
}

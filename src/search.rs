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
static ESTIMATED_TOTAL: AtomicUsize = AtomicUsize::new(0);

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

/// Get current scanned file count (for progress display)
pub fn scanned_count() -> usize {
    SCANNED_COUNT.load(Ordering::Relaxed)
}

/// Estimated total (from previous builds or current scan)
pub fn estimated_total() -> usize {
    ESTIMATED_TOTAL.load(Ordering::Relaxed)
}

fn meta_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("topbar").join("search_index_count.txt")
}

/// A simple in-memory search index built from filenames -> full paths.
use std::collections::HashMap;

pub struct SearchIndex {
    set: Set<Vec<u8>>,
    count: usize,
    /// Map of lowercase extension -> list of full paths
    ext_map: HashMap<String, Vec<String>>,
    /// Map of paths that are from app/program directories (Start Menu, Program Files, etc.)
    app_paths: std::collections::HashSet<String>,
}

impl SearchIndex {
    /// Build an index from the provided roots (walks recursively).
    pub fn build(roots: &[PathBuf]) -> Result<Self> {
        Self::build_with_excludes(roots, &[])
    }

    /// Build an index with exclusion patterns
    pub fn build_with_excludes(roots: &[PathBuf], exclude_patterns: &[String]) -> Result<Self> {
        let mut keys: Vec<String> = Vec::new();

        // Try load a previous estimate (best-effort) so we can show % progress
        let estimate = if let Ok(s) = std::fs::read_to_string(meta_path()) {
            s.trim().parse::<usize>().unwrap_or(0)
        } else { 0 };
        ESTIMATED_TOTAL.store(estimate, Ordering::Relaxed);
        log::info!("Starting search index build (est={} files)...", estimate);
        SCANNED_COUNT.store(0, Ordering::Relaxed);
        IS_BUILDING.store(false, Ordering::Relaxed);

        // Compile glob patterns for exclusion
        let exclude_globs: Vec<glob::Pattern> = exclude_patterns
            .iter()
            .filter_map(|pattern| glob::Pattern::new(pattern).ok())
            .collect();

        // Prepare extension map while scanning
        let mut ext_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut app_paths = std::collections::HashSet::new();

        // Identify common app directories by pattern
        let is_app_directory = |path: &str| -> bool {
            let lower = path.to_lowercase();
            // Program Files directories
            lower.contains("\\program files\\") || 
            lower.contains("\\program files (x86)\\") ||
            // Start Menu locations
            lower.contains("\\start menu\\") ||
            lower.contains("\\microsoft\\windows\\start menu\\") ||
            // AppData program installations
            lower.contains("\\appdata\\local\\programs\\") ||
            lower.contains("\\appdata\\roaming\\") && lower.contains("\\.exe") ||
            // Common installation directories
            lower.contains("\\common files\\") ||
            lower.contains("\\commonprogramfiles\\") ||
            // Portable apps and other app locations
            (lower.contains("\\app\\") && lower.ends_with(".exe")) ||
            (lower.contains("\\application\\") && lower.ends_with(".exe"))
        };

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
                    // record key for fst
                    keys.push(format!("{}\0{}", filename, full));

                    // Check if this path is in an app-related directory
                    if is_app_directory(&full) {
                        app_paths.insert(full.clone());
                    }

                    // record extension -> full path
                    if let Some(ext_os) = entry.path().extension() {
                        if let Some(ext) = ext_os.to_str() {
                            let e = ext.to_lowercase();
                            ext_map.entry(e).or_insert_with(Vec::new).push(full.clone());
                        }
                    }
                }
            }
        }

        log::info!("Collected {} files, sorting...", keys.len());
        // Update estimated total to the actual scanned count before building
        ESTIMATED_TOTAL.store(keys.len(), Ordering::Relaxed);
        IS_BUILDING.store(true, Ordering::Relaxed);
        keys.sort();

        log::info!("Building FST index...");
        let set = Set::from_iter(keys.iter())?;
        let count = set.len();

        // Persist final count for future estimates (best-effort)
        let _ = std::fs::create_dir_all(meta_path().parent().unwrap_or(&PathBuf::from(".")));
        let _ = std::fs::write(meta_path(), format!("{}", count));

        IS_BUILDING.store(false, Ordering::Relaxed);
        log::info!("Search index ready with {} files", count);

        Ok(Self { set, count, ext_map, app_paths })
    }

    /// Return the number of indexed entries
    pub fn count(&self) -> usize {
        self.count
    }

    /// Search for filenames that start with `prefix` (case-insensitive) with smart ranking
    pub fn search_prefix(&self, prefix: &str, limit: usize) -> Vec<String> {
        let q = prefix.to_lowercase();
        let matcher = Str::new(&q).starts_with();
        let mut stream = self.set.search(&matcher).into_stream();

        let mut all_results = Vec::new();
        while let Some(key) = stream.next() {
            if let Ok(s) = std::str::from_utf8(key) {
                if let Some(pos) = s.find('\0') {
                    let filename = &s[..pos];
                    let full_path = &s[pos + 1..];
                    all_results.push((filename.to_string(), full_path.to_string()));
                }
            }
        }

        // Score and sort results by relevance
        let mut scored = all_results
            .into_iter()
            .map(|(filename, path)| {
                let score = calculate_relevance_score(&filename, &path, &q, &self.app_paths);
                (path, score)
            })
            .collect::<Vec<_>>();

        // Sort by score descending, then by path for stable ordering
        scored.sort_by(|a, b| {
            match b.1.partial_cmp(&a.1) {
                Some(std::cmp::Ordering::Equal) | None => a.0.cmp(&b.0),
                Some(ord) => ord,
            }
        });

        scored.into_iter().take(limit).map(|(path, _)| path).collect()
    }

    /// Search by extension (.ext or ext). Case-insensitive. Up to `limit` results
    pub fn search_by_extension(&self, ext: &str, limit: usize) -> Vec<String> {
        let e = ext.trim_start_matches('.').to_lowercase();
        if let Some(v) = self.ext_map.get(&e) {
            return v.iter().take(limit).cloned().collect();
        }
        Vec::new()
    }
}

/// Calculate relevance score for a search result
/// Higher scores = more relevant
fn calculate_relevance_score(filename: &str, path: &str, query: &str, app_paths: &std::collections::HashSet<String>) -> f32 {
    let mut score: f32 = 0.0;

    // 1. Boost for applications/programs (highest priority)
    if app_paths.contains(path) {
        score += 1000.0;
    }

    // 2. Exact filename match (without extension)
    let filename_no_ext = filename.split('.').next().unwrap_or(filename);
    if filename_no_ext.to_lowercase() == query {
        score += 500.0;
    }

    // 3. Filename starts with query (already guaranteed by prefix search)
    // But boost if it's a closer match
    if filename.to_lowercase().starts_with(query) {
        let match_ratio = query.len() as f32 / filename.len() as f32;
        score += 100.0 * match_ratio;
    }

    // 4. Penalty for very long paths (prefer files closer to root)
    let depth = path.matches('\\').count() as f32;
    score -= depth * 2.0;

    // 5. Boost for executable files (.exe, .lnk, .bat)
    if filename.ends_with(".exe") || filename.ends_with(".lnk") || filename.ends_with(".bat") {
        score += 50.0;
    }

    // 6. Boost if filename appears at the very start of path (not in a subdirectory as much)
    if path.to_lowercase().contains(&format!("\\{}", filename.to_lowercase())) {
        let pos = path.to_lowercase().rfind(&format!("\\{}", filename.to_lowercase())).unwrap_or(0);
        let prefix_depth = path[..pos].matches('\\').count();
        score += 50.0 / (prefix_depth as f32 + 1.0);
    }

    score
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

        // Test extension search
        File::create(dir.path().join("image.CR2")).unwrap();
        let idx2 = SearchIndex::build(&[dir.path().to_path_buf()]).unwrap();
        let ext_results = idx2.search_by_extension(".cr2", 10);
        assert!(ext_results.iter().any(|p| p.ends_with("image.CR2")));
    }
}

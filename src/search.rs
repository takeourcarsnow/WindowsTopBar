//! Simple file search index
//!
//! Uses `walkdir` to collect file paths and `fst` to build a compact, fast
//! prefix-searchable set.

use anyhow::Result;
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

pub struct SearchIndex {
    /// Minimal entries: (lowercase filename, lowercase full path, full path)
    entries: Vec<(String, String, String)>,
    /// Map-like set of paths that are from app/program directories (Start Menu, Program Files, etc.)
    app_paths: std::collections::HashSet<String>,
}  

impl SearchIndex {
    /// Build an index from the provided roots (walks recursively).
    pub fn build(roots: &[PathBuf]) -> Result<Self> {
        Self::build_with_excludes(roots, &[])
    }

    /// Build an index with exclusion patterns
    pub fn build_with_excludes(roots: &[PathBuf], exclude_patterns: &[String]) -> Result<Self> {
        // Minimal, fast index: only include common application files and shortcuts
        const MAX_ENTRIES: usize = 10000;
        const MAX_DEPTH: usize = 6;
        let allowed_exts = ["exe", "lnk", "bat", "cmd", "msi", "com", "ps1", "txt", "pdf", "json", "xml", "zip"];

        // Compile glob patterns for exclusion
        let exclude_globs: Vec<glob::Pattern> = exclude_patterns
            .iter()
            .filter_map(|pattern| glob::Pattern::new(pattern).ok())
            .collect();

        let mut entries: Vec<(String, String, String)> = Vec::new();
        let mut app_paths = std::collections::HashSet::new();

        let is_app_directory = |path: &str| -> bool {
            let lower = path.to_lowercase();
            lower.contains("\\program files\\") || lower.contains("\\program files (x86)\\") || lower.contains("\\start menu\\")
        };

        SCANNED_COUNT.store(0, Ordering::Relaxed);
        IS_BUILDING.store(true, Ordering::Relaxed);

        for root in roots {
            log::info!("Indexing directory (shallow): {}", root.display());
            let walker = WalkDir::new(root).follow_links(false).max_depth(MAX_DEPTH).into_iter();

            for entry in walker.filter_map(|e| e.ok()) {
                let path_str = entry.path().to_string_lossy();

                // Check exclusions
                if exclude_globs.iter().any(|p| p.matches(&path_str)) {
                    continue;
                }

                if entry.file_type().is_file() {
                    SCANNED_COUNT.fetch_add(1, Ordering::Relaxed);

                    let full = entry.path().to_string_lossy().to_string();
                    let filename = entry.file_name().to_string_lossy().to_lowercase();
                    if let Some(ext_os) = entry.path().extension() {
                        if let Some(ext) = ext_os.to_str() {
                            let e = ext.to_lowercase();
                            if allowed_exts.contains(&e.as_str()) {
                                let full_lower = full.to_lowercase();
                                entries.push((filename.clone(), full_lower, full.clone()));
                                if is_app_directory(&full) {
                                    app_paths.insert(full.clone());
                                }
                                if entries.len() >= MAX_ENTRIES {
                                    log::info!("Reached max entries ({}), stopping early", MAX_ENTRIES);
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            if entries.len() >= MAX_ENTRIES { break; }
        }

        IS_BUILDING.store(false, Ordering::Relaxed);
        log::info!("Minimal search index built with {} entries", entries.len());

        Ok(Self { entries, app_paths })
    }

    /// Return the number of indexed entries
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Search for filenames that start with `prefix` (case-insensitive) with smart ranking
    pub fn search_prefix(&self, prefix: &str, limit: usize) -> Vec<String> {
        let q = prefix.to_lowercase();
        let mut best: Vec<(f32, String)> = Vec::new();

        for (filename, _path_lower, full) in &self.entries {
            if filename.starts_with(&q) {
                let score = calculate_relevance_score(filename, full, &q, &self.app_paths);
                if best.len() < limit {
                    best.push((score, full.clone()));
                } else {
                    // replace min if better
                    let mut min_idx = 0usize;
                    let mut min_score = best[0].0;
                    for i in 1..best.len() {
                        if best[i].0 < min_score {
                            min_score = best[i].0;
                            min_idx = i;
                        }
                    }
                    if score > min_score {
                        best[min_idx] = (score, full.clone());
                    }
                }
            }
        }

        best.sort_by(|a, b| match b.0.partial_cmp(&a.0) {
            Some(ord) => ord,
            None => std::cmp::Ordering::Equal,
        });

        best.into_iter().map(|(_, path)| path).collect()
    }

    /// Simple contains-based search (case-insensitive) that matches query anywhere in filename or path
    pub fn search_query(&self, query: &str, limit: usize) -> Vec<String> {
        let q = query.to_lowercase();

        // Maintain a small bounded collection of best candidates to avoid allocating and sorting huge result sets
        let mut best: Vec<(f32, String)> = Vec::new();

        for (filename, path_lower, full) in &self.entries {
            if filename.contains(&q) || filename.split('.').next().unwrap_or("").contains(&q) || path_lower.contains(&q) {
                let score = calculate_relevance_score(filename, full, &q, &self.app_paths);

                if best.len() < limit {
                    best.push((score, full.clone()));
                } else {
                    // find smallest score in current best and replace if this is better
                    let mut min_idx = 0usize;
                    let mut min_score = best[0].0;
                    for i in 1..best.len() {
                        if best[i].0 < min_score {
                            min_score = best[i].0;
                            min_idx = i;
                        }
                    }
                    if score > min_score {
                        best[min_idx] = (score, full.clone());
                    }
                }
            }
        }

        // Final sort of small set by score descending, then path
        best.sort_by(|a, b| match b.0.partial_cmp(&a.0) {
            Some(ord) => ord,
            None => std::cmp::Ordering::Equal,
        });

        best.into_iter().map(|(_, path)| path).collect()
    }

    /// Search by extension (.ext or ext). Case-insensitive. Up to `limit` results
    pub fn search_by_extension(&self, ext: &str, limit: usize) -> Vec<String> {
        let e = ext.trim_start_matches('.').to_lowercase();
        let mut res: Vec<String> = Vec::new();
        for (_filename, _path_lower, full) in &self.entries {
            if let Some(ext_os) = std::path::Path::new(full).extension() {
                if let Some(exts) = ext_os.to_str() {
                    if exts.to_lowercase() == e {
                        res.push(full.clone());
                        if res.len() >= limit { break; }
                    }
                }
            }
        }
        res
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

    // 5. Boost for executable and script files
    if filename.ends_with(".exe") || filename.ends_with(".lnk") || filename.ends_with(".bat") || filename.ends_with(".cmd") || filename.ends_with(".ps1") {
        score += 50.0;
    }
    
    // 6. Boost for document and archive files
    if filename.ends_with(".txt") || filename.ends_with(".pdf") || filename.ends_with(".json") || filename.ends_with(".xml") || filename.ends_with(".zip") {
        score += 20.0;
    }

    // 7. Boost if filename appears at the very start of path (not in a subdirectory as much)
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
        File::create(dir.path().join("Hello.exe")).unwrap();
        File::create(dir.path().join("hello_world.exe")).unwrap();
        File::create(dir.path().join("Other.exe")).unwrap();

        let idx = SearchIndex::build(&[dir.path().to_path_buf()]).unwrap();
        assert!(idx.count() >= 3);

        let results = idx.search_prefix("hel", 10);
        assert!(results.iter().any(|p| p.ends_with("Hello.exe")));
        assert!(results.iter().any(|p| p.ends_with("hello_world.exe")));

        // contains-based search should find substrings inside filenames
        let results_contains = idx.search_query("llo", 10);
        assert!(results_contains.iter().any(|p| p.ends_with("Hello.exe")));

        // Test extension search
        File::create(dir.path().join("image.EXE")).unwrap();
        let idx2 = SearchIndex::build(&[dir.path().to_path_buf()]).unwrap();
        let ext_results = idx2.search_by_extension(".exe", 10);
        assert!(ext_results.iter().any(|p| p.ends_with("image.EXE")));
    }
}

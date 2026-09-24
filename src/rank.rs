use crate::analyze::FileEntry;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

const DAMPING: f64 = 0.85;
const ITERATIONS: usize = 24;

/// Scores every entry by its centrality in the import graph, so that a token
/// budget can be spent on the files the rest of the repo actually depends on.
///
/// When `focus` names a file, the random-walk restart is concentrated there
/// instead of spread uniformly, which ranks by proximity to that file rather
/// than by global importance.
pub fn score_entries(entries: &mut [FileEntry], focus: Option<&Path>) {
    if entries.is_empty() {
        return;
    }

    let edges = build_edges(entries);
    let n = entries.len();

    let teleport = teleport_vector(entries, focus);
    let mut scores = teleport.clone();

    // Out-degree per node, used to split each node's rank among its targets.
    let out_degree: Vec<usize> = edges.iter().map(|targets| targets.len()).collect();

    for _ in 0..ITERATIONS {
        let mut next = vec![0.0; n];
        let mut dangling = 0.0;

        for i in 0..n {
            if out_degree[i] == 0 {
                dangling += scores[i];
                continue;
            }
            let share = scores[i] / out_degree[i] as f64;
            for &target in &edges[i] {
                next[target] += share;
            }
        }

        // A file that imports nothing would otherwise leak its rank out of the
        // graph; redistribute it along the teleport vector instead.
        for i in 0..n {
            next[i] = DAMPING * (next[i] + dangling * teleport[i]) + (1.0 - DAMPING) * teleport[i];
        }

        scores = next;
    }

    for (entry, score) in entries.iter_mut().zip(scores) {
        entry.score = score;
    }
}

fn teleport_vector(entries: &[FileEntry], focus: Option<&Path>) -> Vec<f64> {
    let n = entries.len();
    let uniform = 1.0 / n as f64;

    let Some(focus) = focus else {
        return vec![uniform; n];
    };

    let focus_norm = normalize(focus);
    let matched: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            let path = normalize(&e.path);
            path == focus_norm || path.starts_with(&focus_norm)
        })
        .map(|(i, _)| i)
        .collect();

    if matched.is_empty() {
        return vec![uniform; n];
    }

    let mut teleport = vec![0.0; n];
    let weight = 1.0 / matched.len() as f64;
    for i in matched {
        teleport[i] = weight;
    }
    teleport
}

/// Adjacency list of importer index -> imported indices.
fn build_edges(entries: &[FileEntry]) -> Vec<Vec<usize>> {
    let index = PathIndex::build(entries);

    entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let mut targets: Vec<usize> = entry
                .imports
                .iter()
                .filter_map(|import| index.resolve(&entry.path, import))
                // A file importing its own name adds nothing to the graph.
                .filter(|target| *target != i)
                .collect();
            targets.sort_unstable();
            targets.dedup();
            targets
        })
        .collect()
}

/// Maps the shapes an import string can take onto indices into `entries`.
struct PathIndex {
    /// Normalized path with the extension removed, plus directory aliases for
    /// `mod.rs`/`index.ts`/`__init__.py`. Used for exact relative resolution.
    by_path: HashMap<PathBuf, usize>,
    /// Every trailing slice of each file's path, so that an import written
    /// from a crate or package root still matches. Slices shared by more than
    /// one file are dropped rather than guessed at.
    by_suffix: HashMap<String, usize>,
}

impl PathIndex {
    fn build(entries: &[FileEntry]) -> Self {
        let mut by_path = HashMap::new();
        let mut by_suffix: HashMap<String, usize> = HashMap::new();
        let mut counts: HashMap<String, usize> = HashMap::new();

        for (i, entry) in entries.iter().enumerate() {
            let normalized = normalize(&entry.path);
            let stripped = strip_extension(&normalized);
            by_path.insert(stripped.clone(), i);

            let mut keys = vec![stripped.clone()];

            // `mod.rs`, `index.ts` and `__init__.py` stand for their directory.
            if let Some(stem) = stripped.file_name().and_then(|s| s.to_str())
                && matches!(stem, "mod" | "index" | "__init__")
                && let Some(parent) = stripped.parent()
                && !parent.as_os_str().is_empty()
            {
                by_path.insert(parent.to_path_buf(), i);
                keys.push(parent.to_path_buf());
            }

            for key in keys {
                let as_str = key.to_string_lossy().replace('\\', "/");
                for suffix in path_suffixes(&as_str) {
                    *counts.entry(suffix.clone()).or_insert(0) += 1;
                    by_suffix.insert(suffix, i);
                }
            }
        }

        by_suffix.retain(|suffix, _| counts.get(suffix) == Some(&1));

        Self { by_path, by_suffix }
    }

    fn resolve(&self, importer: &Path, import: &str) -> Option<usize> {
        let cleaned = import.trim();
        if cleaned.is_empty() {
            return None;
        }

        // A relative import is the only kind that can be resolved exactly.
        if cleaned.starts_with('.') {
            let base = importer.parent().unwrap_or(Path::new(""));
            let joined = normalize(&base.join(cleaned));
            if let Some(i) = self.by_path.get(&strip_extension(&joined)) {
                return Some(*i);
            }
        }

        // `crate::parser::Symbol`, `app.services.auth`, `java.util.List`: the
        // separator differs but the shape is the same. The module part can be
        // anywhere in the path, since imports may carry both a root prefix
        // (`crate`) and a trailing item name (`Symbol`), so every contiguous
        // run of segments is tried, longest first.
        let as_path = cleaned.replace("::", "/").replace(['\\', '.'], "/");

        for candidate in segment_runs(&as_path) {
            if let Some(i) = self.by_suffix.get(&candidate) {
                return Some(*i);
            }
        }

        None
    }
}

/// `a/b/c` yields `a/b/c`, `b/c`, `c`: the trailing slices of a path.
fn path_suffixes(path: &str) -> Vec<String> {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    (0..parts.len())
        .map(|start| parts[start..].join("/"))
        .collect()
}

/// Every contiguous run of segments in `path`, longest first so that the most
/// specific match wins.
fn segment_runs(path: &str) -> Vec<String> {
    let parts: Vec<&str> = path
        .split('/')
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.starts_with('{'))
        .collect();

    let mut runs: Vec<String> = Vec::new();
    for len in (1..=parts.len()).rev() {
        for start in 0..=(parts.len() - len) {
            runs.push(parts[start..start + len].join("/"));
        }
    }
    runs
}

fn strip_extension(path: &Path) -> PathBuf {
    match path.extension() {
        Some(_) => path.with_extension(""),
        None => path.to_path_buf(),
    }
}

/// Resolves `.` and `..` lexically and drops any leading `./`, so that paths
/// built from an import string compare equal to paths seen while walking.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Keeps the highest-scoring entries whose rendered size fits `max_tokens`.
///
/// Returns the kept entries in their original order along with how many were
/// dropped, so the map can say what it left out rather than silently truncating.
pub fn apply_budget<F>(
    entries: Vec<FileEntry>,
    max_tokens: usize,
    estimate: F,
) -> (Vec<FileEntry>, usize)
where
    F: Fn(&FileEntry) -> usize,
{
    let total = entries.len();

    let mut ranked: Vec<(usize, FileEntry)> = entries.into_iter().enumerate().collect();
    // Highest score first, with path as a tiebreak so equal scores are stable.
    ranked.sort_by(|a, b| {
        b.1.score
            .partial_cmp(&a.1.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.path.cmp(&b.1.path))
    });

    let mut spent = 0usize;
    let mut kept: Vec<(usize, FileEntry)> = Vec::new();

    for (original_index, entry) in ranked {
        let cost = estimate(&entry);
        if spent + cost > max_tokens && !kept.is_empty() {
            continue;
        }
        spent += cost;
        kept.push((original_index, entry));
    }

    kept.sort_by_key(|(i, _)| *i);
    let dropped = total - kept.len();

    (kept.into_iter().map(|(_, e)| e).collect(), dropped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Language;

    fn entry(path: &str, imports: &[&str]) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            language: Language::Typescript,
            line_count: 10,
            symbols: Vec::new(),
            imports: imports.iter().map(|s| s.to_string()).collect(),
            score: 0.0,
        }
    }

    #[test]
    fn test_widely_imported_file_ranks_highest() {
        let mut entries = vec![
            entry("src/core.ts", &[]),
            entry("src/a.ts", &["./core"]),
            entry("src/b.ts", &["./core"]),
            entry("src/c.ts", &["./core"]),
        ];

        score_entries(&mut entries, None);

        let core = entries
            .iter()
            .find(|e| e.path.ends_with("core.ts"))
            .unwrap();
        let leaf = entries.iter().find(|e| e.path.ends_with("a.ts")).unwrap();
        assert!(
            core.score > leaf.score,
            "core {} should outrank leaf {}",
            core.score,
            leaf.score
        );
    }

    #[test]
    fn test_scores_sum_to_one() {
        let mut entries = vec![
            entry("src/core.ts", &[]),
            entry("src/a.ts", &["./core"]),
            entry("src/b.ts", &["./core", "./a"]),
        ];

        score_entries(&mut entries, None);

        let total: f64 = entries.iter().map(|e| e.score).sum();
        assert!((total - 1.0).abs() < 1e-6, "scores summed to {total}");
    }

    #[test]
    fn test_focus_shifts_ranking() {
        let mut entries = vec![
            entry("src/core.ts", &[]),
            entry("src/a.ts", &["./core"]),
            entry("src/lonely.ts", &[]),
        ];

        score_entries(&mut entries, Some(Path::new("src/lonely.ts")));

        let lonely = entries
            .iter()
            .find(|e| e.path.ends_with("lonely.ts"))
            .unwrap();
        assert!(
            lonely.score > 0.5,
            "focused file should dominate, got {}",
            lonely.score
        );
    }

    #[test]
    fn test_rust_module_import_resolves() {
        let mut entries = vec![entry("src/parser.rs", &[]), entry("src/main.rs", &[])];
        entries[1].imports = vec!["crate::parser".to_string()];

        let edges = build_edges(&entries);
        assert_eq!(
            edges[1],
            vec![0],
            "crate::parser should resolve to parser.rs"
        );
    }

    #[test]
    fn test_index_file_stands_for_its_directory() {
        let entries = vec![
            entry("src/widgets/index.ts", &[]),
            entry("src/app.ts", &["./widgets"]),
        ];

        let edges = build_edges(&entries);
        assert_eq!(edges[1], vec![0]);
    }

    #[test]
    fn test_ambiguous_stem_does_not_invent_an_edge() {
        let entries = vec![
            entry("a/util.ts", &[]),
            entry("b/util.ts", &[]),
            entry("src/app.ts", &["util"]),
        ];

        let edges = build_edges(&entries);
        assert!(
            edges[2].is_empty(),
            "ambiguous stem should not resolve, got {:?}",
            edges[2]
        );
    }

    #[test]
    fn test_parent_dir_import_normalizes() {
        let entries = vec![
            entry("src/core.ts", &[]),
            entry("src/nested/deep.ts", &["../core"]),
        ];

        let edges = build_edges(&entries);
        assert_eq!(edges[1], vec![0]);
    }

    #[test]
    fn test_rust_use_with_trailing_item_resolves_to_the_module() {
        // `crate::analyze::FileEntry` names a module and then an item inside
        // it; only the module part corresponds to a file.
        let mut entries = vec![entry("src/analyze.rs", &[]), entry("src/rank.rs", &[])];
        entries[1].imports = vec!["crate::analyze::FileEntry".to_string()];

        let edges = build_edges(&entries);
        assert_eq!(edges[1], vec![0]);
    }

    #[test]
    fn test_braced_rust_use_resolves() {
        let mut entries = vec![entry("src/parser.rs", &[]), entry("src/analyze.rs", &[])];
        entries[1].imports = vec!["crate::parser::{self, Symbol}".to_string()];

        let edges = build_edges(&entries);
        assert_eq!(edges[1], vec![0]);
    }

    #[test]
    fn test_self_import_creates_no_edge() {
        let mut entries = vec![entry("src/parser.rs", &[])];
        entries[0].imports = vec!["crate::parser::Symbol".to_string()];

        let edges = build_edges(&entries);
        assert!(edges[0].is_empty(), "got {:?}", edges[0]);
    }

    #[test]
    fn test_external_import_creates_no_edge() {
        let mut entries = vec![entry("src/app.ts", &[]), entry("src/core.ts", &[])];
        entries[0].imports = vec!["react".to_string(), "std::collections::HashMap".to_string()];

        let edges = build_edges(&entries);
        assert!(edges[0].is_empty(), "got {:?}", edges[0]);
    }

    #[test]
    fn test_python_dotted_import_resolves_to_a_nested_file() {
        let mut entries = vec![
            entry("app/services/auth.py", &[]),
            entry("app/main.py", &[]),
        ];
        entries[1].imports = vec!["app.services.auth".to_string()];

        let edges = build_edges(&entries);
        assert_eq!(edges[1], vec![0]);
    }

    #[test]
    fn test_budget_keeps_highest_scoring_and_reports_drops() {
        let mut entries = vec![entry("a.ts", &[]), entry("b.ts", &[]), entry("c.ts", &[])];
        entries[0].score = 0.1;
        entries[1].score = 0.7;
        entries[2].score = 0.2;

        let (kept, dropped) = apply_budget(entries, 20, |_| 10);

        assert_eq!(dropped, 1);
        let paths: Vec<String> = kept.iter().map(|e| e.path.display().to_string()).collect();
        assert_eq!(
            paths,
            vec!["b.ts", "c.ts"],
            "kept should stay in path order"
        );
    }

    #[test]
    fn test_budget_always_keeps_at_least_one_entry() {
        let entries = vec![entry("huge.ts", &[])];
        let (kept, dropped) = apply_budget(entries, 1, |_| 10_000);

        assert_eq!(
            kept.len(),
            1,
            "a budget smaller than one file still yields it"
        );
        assert_eq!(dropped, 0);
    }
}

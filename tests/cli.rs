//! End-to-end tests: run the real binary against a checked-in fixture repo and
//! compare whole outputs against golden files.
//!
//! Unit tests cover each query in isolation, but only a full run catches
//! regressions in how walking, ranking, budgeting and rendering compose. Set
//! `REPOMAP_UPDATE_GOLDEN=1` to rewrite the golden files after an intentional
//! change, then read the diff before committing it.

use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE: &str = "tests/fixture";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_repomap"))
        .current_dir(repo_root())
        .args(args)
        .output()
        .expect("failed to run repomap");

    assert!(
        output.status.success(),
        "repomap {:?} exited with {:?}\nstderr: {}",
        args,
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("stdout was not utf-8")
}

fn run_expecting_failure(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_repomap"))
        .current_dir(repo_root())
        .args(args)
        .output()
        .expect("failed to run repomap");

    assert!(
        !output.status.success(),
        "repomap {args:?} unexpectedly succeeded"
    );

    String::from_utf8_lossy(&output.stderr).to_string()
}

fn assert_golden(name: &str, actual: &str) {
    let path = repo_root().join("tests/golden").join(name);

    if std::env::var("REPOMAP_UPDATE_GOLDEN").is_ok() {
        std::fs::write(&path, actual).expect("failed to write golden file");
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden file {}\nrerun with REPOMAP_UPDATE_GOLDEN=1 to create it",
            path.display()
        )
    });

    assert_eq!(
        expected,
        actual,
        "output drifted from {}\nrerun with REPOMAP_UPDATE_GOLDEN=1 to update",
        path.display()
    );
}

#[test]
fn test_default_markdown_output() {
    assert_golden("default.md", &run(&[FIXTURE]));
}

#[test]
fn test_minimal_output() {
    assert_golden("minimal.md", &run(&["-m", FIXTURE]));
}

#[test]
fn test_no_signatures_output() {
    assert_golden("no-signatures.md", &run(&["--no-signatures", FIXTURE]));
}

#[test]
fn test_summary_output() {
    assert_golden("summary.md", &run(&["-s", FIXTURE]));
}

#[test]
fn test_json_output() {
    assert_golden("map.json", &run(&["-f", "json", FIXTURE]));
}

#[test]
fn test_json_is_valid_and_carries_signatures() {
    let raw = run(&["-f", "json", FIXTURE]);
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("output was not valid JSON");

    assert_eq!(parsed["root"], FIXTURE);
    let files = parsed["files"].as_array().expect("files array");
    assert_eq!(files.len(), 5);

    let core = files
        .iter()
        .find(|f| f["path"].as_str().unwrap_or("").ends_with("core.ts"))
        .expect("core.ts missing");
    let make_store = core["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "makeStore")
        .expect("makeStore missing");

    assert_eq!(make_store["exported"], true);
    assert!(
        make_store["signature"]
            .as_str()
            .is_some_and(|s| s.contains("makeStore")),
        "signature missing: {make_store:?}"
    );
}

#[test]
fn test_no_locals_drops_bindings_inside_functions() {
    let full = run(&[FIXTURE]);
    let surface = run(&["--no-locals", FIXTURE]);

    // The fixture declares `const store` inside `run`. With signatures on,
    // the label carries the whole binding.
    assert!(full.contains("run > const store"), "got:\n{full}");
    assert!(!surface.contains("run > const store"), "got:\n{surface}");
    assert!(
        surface.contains("export const run"),
        "the function itself should remain:\n{surface}"
    );
    assert!(surface.len() < full.len());
}

#[test]
fn test_json_marks_locals() {
    let raw = run(&["-f", "json", FIXTURE]);
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

    let app = parsed["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"].as_str().unwrap_or("").ends_with("app.ts"))
        .expect("app.ts");

    let store = app["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "store")
        .expect("store");

    assert_eq!(store["local"], true);
    assert_eq!(store["parent"], "run");
}

#[test]
fn test_output_is_deterministic_across_runs() {
    // Files are parsed in parallel, so a stable ordering is not automatic.
    let first = run(&[FIXTURE]);
    for _ in 0..4 {
        assert_eq!(first, run(&[FIXTURE]), "output differed between runs");
    }
}

#[test]
fn test_ranking_prefers_the_widely_imported_file() {
    let raw = run(&["-f", "json", FIXTURE]);
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");
    let files = parsed["files"].as_array().unwrap();

    let score_of = |suffix: &str| -> f64 {
        files
            .iter()
            .find(|f| f["path"].as_str().unwrap_or("").ends_with(suffix))
            .and_then(|f| f["score"].as_f64())
            .unwrap_or_else(|| panic!("{suffix} had no score"))
    };

    assert!(
        score_of("core.ts") > score_of("app.ts"),
        "core.ts is imported by app.ts and should outrank it"
    );
}

#[test]
fn test_max_tokens_drops_lower_ranked_files_and_says_so() {
    let full = run(&[FIXTURE]);
    let budgeted = run(&["--max-tokens", "120", FIXTURE]);

    assert!(budgeted.len() < full.len(), "budget did not shrink the map");
    assert!(
        budgeted.contains("**Omitted:**"),
        "budgeted map should report what it dropped:\n{budgeted}"
    );
    assert!(
        budgeted.contains("core.ts"),
        "the highest-ranked file should survive the budget:\n{budgeted}"
    );
}

#[test]
fn test_max_tokens_is_respected() {
    let budgeted = run(&["--max-tokens", "150", FIXTURE]);
    // The budget governs file bodies, not the header, so allow a small margin.
    let estimate = budgeted.len() / 4;
    assert!(
        estimate < 300,
        "budget of 150 produced roughly {estimate} tokens"
    );
}

#[test]
fn test_focus_reranks_around_the_named_file() {
    let raw = run(&[
        "-f",
        "json",
        "--focus",
        "tests/fixture/src/util.py",
        FIXTURE,
    ]);
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");
    let files = parsed["files"].as_array().unwrap();

    let util = files
        .iter()
        .find(|f| f["path"].as_str().unwrap_or("").ends_with("util.py"))
        .and_then(|f| f["score"].as_f64())
        .expect("util.py score");

    assert!(
        util > 0.5,
        "the focused file should dominate the ranking, got {util}"
    );
}

#[test]
fn test_language_override_applies_one_grammar_to_every_file() {
    let out = run(&["-l", "python", FIXTURE]);

    // Every file is parsed as Python, so the Python file still resolves and
    // the TypeScript files are read through the wrong grammar rather than
    // being skipped. What matters is that Python-specific structure appears
    // and TypeScript-specific structure does not.
    assert!(out.contains("util.py"));
    assert!(out.contains("Widget > def render"), "got:\n{out}");
    assert!(
        !out.contains("interface Store"),
        "TypeScript interfaces cannot come from the Python grammar:\n{out}"
    );
    assert!(out.contains("```python"), "every fence should say python");
}

#[test]
fn test_exclude_glob_skips_matching_files() {
    let out = run(&["-e", "*.ts", FIXTURE]);

    assert!(out.contains("util.py"));
    assert!(!out.contains("core.ts"), "excluded file appeared:\n{out}");
}

#[test]
fn test_depth_limits_traversal() {
    let out = run(&["--depth", "1", FIXTURE]);

    assert!(out.contains("README.md"));
    assert!(
        !out.contains("core.ts"),
        "depth 1 should not reach src/:\n{out}"
    );
}

#[test]
fn test_since_reports_a_bad_ref_instead_of_mapping_everything() {
    let stderr = run_expecting_failure(&["--since", "definitely-not-a-ref", FIXTURE]);

    assert!(
        stderr.contains("definitely-not-a-ref"),
        "error should name the bad ref, got: {stderr}"
    );
}

#[test]
fn test_since_head_on_a_clean_tree_maps_nothing() {
    // The fixture is committed, so nothing has changed against HEAD.
    let out = run(&["--since", "HEAD", FIXTURE]);
    assert!(out.contains("**Files:** 0"), "got:\n{out}");
}

#[test]
fn test_claude_flag_updates_only_the_marked_section() {
    let dir = tempfile::tempdir().expect("temp dir");
    let target = dir.path().join("CLAUDE.md");
    std::fs::write(&target, "# Project notes\n\nKeep me.\n").expect("seed file");

    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_repomap"))
            .current_dir(repo_root())
            .args(["--claude", "-o"])
            .arg(&target)
            .arg(FIXTURE)
            .output()
            .expect("failed to run repomap");
        assert!(output.status.success());
    }

    let content = std::fs::read_to_string(&target).expect("read back");

    assert!(content.contains("Keep me."), "existing content was lost");
    assert_eq!(
        content.matches("<!-- REPOMAP START -->").count(),
        1,
        "running twice should replace the section, not append a second one"
    );
    assert!(content.contains("<details>"));
}

#[test]
fn test_generated_files_are_never_mapped() {
    let out = run(&[FIXTURE]);
    assert!(!out.contains("repomap.md"));
    assert!(!out.contains("CLAUDE.md"));
}

#[test]
fn test_fixture_paths_are_relative_to_the_given_root() {
    let out = run(&[FIXTURE]);
    assert!(out.contains(&format!(
        "## {}",
        Path::new(FIXTURE).join("README.md").display()
    )));
}

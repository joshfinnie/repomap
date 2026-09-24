use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Paths that differ from `git_ref` in the working tree, as absolute paths.
///
/// Tracked modifications come from `git diff`, and new files that were never
/// committed are added on top: both are "changed since the ref" as far as
/// anyone reading a map is concerned.
///
/// Shells out to `git` rather than linking a git library: the tool already
/// assumes a repo-shaped directory, and this keeps the dependency surface flat.
pub fn changed_files(root: &str, git_ref: &str) -> Result<HashSet<PathBuf>> {
    let repo_root = repo_root(root)?;

    // Both commands run from the repository root: `git diff` reports
    // root-relative paths but `git ls-files` reports paths relative to the
    // working directory, so anywhere else the two disagree.
    let diff = run_git(
        &repo_root,
        &["diff", "--name-only", "--diff-filter=d", git_ref],
        &format!("git diff against {git_ref}"),
    )?;

    let untracked = run_git(
        &repo_root,
        &["ls-files", "--others", "--exclude-standard"],
        "git ls-files for untracked paths",
    )?;

    Ok([diff, untracked]
        .iter()
        .flat_map(|out| out.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        // git reports paths from the repository root, not from `root`.
        .map(|line| repo_root.join(line))
        .collect())
}

fn run_git(root: &Path, args: &[&str], what: &str) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {what}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!("{what} failed: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn repo_root(root: &str) -> Result<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .context("failed to locate the git repository root")?;

    if !output.status.success() {
        bail!("{} is not inside a git repository", root);
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(path))
}

/// True when `path` is in `changed`, comparing canonical paths so that a
/// relative walk path still matches git's repo-root-relative output.
pub fn is_changed(changed: &HashSet<PathBuf>, path: &Path) -> bool {
    let canonical = path.canonicalize().ok();

    changed.iter().any(|candidate| {
        if candidate == path {
            return true;
        }
        match (&canonical, candidate.canonicalize().ok()) {
            (Some(a), Some(b)) => *a == b,
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git command failed to start");
        assert!(
            status.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&status.stderr)
        );
    }

    fn init_repo() -> TempDir {
        let dir = TempDir::new().expect("temp dir");
        git(dir.path(), &["init", "-q"]);
        git(dir.path(), &["config", "user.email", "test@example.com"]);
        git(dir.path(), &["config", "user.name", "Test"]);
        dir
    }

    #[test]
    fn test_changed_files_lists_only_the_modified_file() {
        let dir = init_repo();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
        std::fs::write(dir.path().join("b.rs"), "fn b() {}").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-qm", "init"]);

        std::fs::write(dir.path().join("a.rs"), "fn a() { let x = 1; }").unwrap();

        let root = dir.path().to_str().unwrap();
        let changed = changed_files(root, "HEAD").expect("diff failed");

        assert!(is_changed(&changed, &dir.path().join("a.rs")));
        assert!(!is_changed(&changed, &dir.path().join("b.rs")));
    }

    #[test]
    fn test_unknown_ref_is_an_error() {
        let dir = init_repo();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-qm", "init"]);

        let root = dir.path().to_str().unwrap();
        assert!(changed_files(root, "no-such-ref").is_err());
    }

    #[test]
    fn test_untracked_files_count_as_changed() {
        let dir = init_repo();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-qm", "init"]);

        // Never committed, so `git diff` alone would not mention it.
        std::fs::write(dir.path().join("brand_new.rs"), "fn new() {}").unwrap();

        let root = dir.path().to_str().unwrap();
        let changed = changed_files(root, "HEAD").expect("diff failed");

        assert!(is_changed(&changed, &dir.path().join("brand_new.rs")));
        assert!(!is_changed(&changed, &dir.path().join("a.rs")));
    }

    #[test]
    fn test_untracked_paths_resolve_when_run_from_a_subdirectory() {
        // `git ls-files --others` reports paths relative to where it runs, so
        // a nested root used to produce paths that matched nothing.
        let dir = init_repo();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/tracked.rs"), "fn t() {}").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-qm", "init"]);
        std::fs::write(dir.path().join("sub/fresh.rs"), "fn f() {}").unwrap();

        let sub = dir.path().join("sub");
        let changed = changed_files(sub.to_str().unwrap(), "HEAD").expect("diff failed");

        assert!(
            is_changed(&changed, &sub.join("fresh.rs")),
            "expected sub/fresh.rs in {changed:?}"
        );
    }

    #[test]
    fn test_gitignored_files_are_not_reported() {
        let dir = init_repo();
        std::fs::write(dir.path().join(".gitignore"), "ignored.rs\n").unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-qm", "init"]);
        std::fs::write(dir.path().join("ignored.rs"), "fn ignored() {}").unwrap();

        let root = dir.path().to_str().unwrap();
        let changed = changed_files(root, "HEAD").expect("diff failed");

        assert!(!is_changed(&changed, &dir.path().join("ignored.rs")));
    }

    #[test]
    fn test_deleted_files_are_excluded() {
        let dir = init_repo();
        std::fs::write(dir.path().join("gone.rs"), "fn gone() {}").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-qm", "init"]);
        std::fs::remove_file(dir.path().join("gone.rs")).unwrap();

        let root = dir.path().to_str().unwrap();
        let changed = changed_files(root, "HEAD").expect("diff failed");

        assert!(changed.is_empty(), "deleted paths should be filtered out");
    }
}

use anyhow::Result;
use ignore::overrides::OverrideBuilder;
use ignore::{Walk, WalkBuilder};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Creates a configured iterator for traversing the repository.
///
/// `excludes` are glob patterns (e.g. `target`, `*.test.ts`) matched against
/// paths relative to `root` and skipped, same semantics as a `.gitignore` line.
pub fn create_walker(root: &str, depth: Option<usize>, excludes: &[String]) -> Result<Walk> {
    let mut builder = WalkBuilder::new(root);

    if let Some(d) = depth {
        builder.max_depth(Some(d));
    }

    if !excludes.is_empty() {
        let mut override_builder = OverrideBuilder::new(root);
        for pattern in excludes {
            let glob = if let Some(stripped) = pattern.strip_prefix('!') {
                stripped.to_string()
            } else {
                format!("!{}", pattern)
            };
            override_builder.add(&glob)?;
        }
        builder.overrides(override_builder.build()?);
    }

    Ok(builder.git_ignore(true).hidden(true).build())
}

pub fn is_binary(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return true,
    };

    let mut buffer = [0u8; 1024];
    let n = file.read(&mut buffer).unwrap_or(0);

    buffer[..n].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn test_is_binary() {
        let mut text_file = NamedTempFile::new().unwrap();
        writeln!(text_file, "This is just some text").unwrap();
        assert!(!is_binary(text_file.path()));

        let mut bin_file = NamedTempFile::new().unwrap();
        bin_file.write_all(&[0, 155, 20, 0, 255]).unwrap();
        assert!(is_binary(bin_file.path()));
    }

    #[test]
    fn test_create_walker_excludes_glob() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("keep.rs"), "fn main() {}").unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/skip.rs"), "fn skip() {}").unwrap();

        let excludes = vec!["target".to_string()];
        let walker = create_walker(dir.path().to_str().unwrap(), None, &excludes).unwrap();

        let paths: Vec<String> = walker
            .filter_map(|e| e.ok())
            .map(|e| e.path().display().to_string())
            .collect();

        assert!(paths.iter().any(|p| p.ends_with("keep.rs")));
        assert!(!paths.iter().any(|p| p.contains("target")));
    }
}

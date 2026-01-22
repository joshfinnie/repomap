use ignore::{Walk, WalkBuilder};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Creates a configured iterator for traversing the repository.
pub fn create_walker(root: &str, depth: Option<usize>, excludes: &[String]) -> Walk {
    let mut builder = WalkBuilder::new(root);

    if let Some(d) = depth {
        builder.max_depth(Some(d));
    }

    for pattern in excludes {
        builder.add_custom_ignore_filename(pattern);
    }

    builder.git_ignore(true).hidden(true).build()
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
    use tempfile::NamedTempFile;

    #[test]
    fn test_is_binary() {
        let mut text_file = NamedTempFile::new().unwrap();
        writeln!(text_file, "This is just some text").unwrap();
        assert!(!is_binary(text_file.path()));

        let mut bin_file = NamedTempFile::new().unwrap();
        bin_file.write_all(&[0, 155, 20, 0, 255]).unwrap();
        assert!(is_binary(bin_file.path()));
    }
}

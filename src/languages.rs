use clap::ValueEnum;
use std::path::Path;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum Language {
    Rust,
    Python,
    Go,
    Javascript,
    Typescript,
    Tsx,
    Markdown,
}

pub fn infer_language(path: &Path) -> Option<Language> {
    match path.extension()?.to_str()? {
        "rs" => Some(Language::Rust),
        "py" => Some(Language::Python),
        "go" => Some(Language::Go),
        "js" | "jsx" => Some(Language::Javascript),
        "ts" => Some(Language::Typescript),
        "tsx" => Some(Language::Tsx),
        "md" => Some(Language::Markdown),
        _ => None,
    }
}

pub fn get_ts_language(lang: Language) -> tree_sitter::Language {
    match lang {
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::Go => tree_sitter_go::LANGUAGE.into(),
        Language::Javascript => tree_sitter_javascript::LANGUAGE.into(),
        Language::Typescript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Language::Markdown => tree_sitter_md::LANGUAGE.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_infer_language() {
        assert_eq!(infer_language(Path::new("main.rs")), Some(Language::Rust));
        assert_eq!(
            infer_language(Path::new("README.md")),
            Some(Language::Markdown)
        );
        assert_eq!(
            infer_language(Path::new("script.py")),
            Some(Language::Python)
        );
        assert_eq!(infer_language(Path::new("photo.jpg")), None);
    }
}

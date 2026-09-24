use clap::ValueEnum;
use std::path::Path;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, ValueEnum, Debug)]
pub enum Language {
    Rust,
    Python,
    Go,
    Javascript,
    Typescript,
    Tsx,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
    Swift,
    Kotlin,
    Markdown,
}

pub fn infer_language(path: &Path) -> Option<Language> {
    match path.extension()?.to_str()? {
        "rs" => Some(Language::Rust),
        "py" | "pyi" => Some(Language::Python),
        "go" => Some(Language::Go),
        "js" | "jsx" | "mjs" | "cjs" => Some(Language::Javascript),
        "ts" | "mts" | "cts" => Some(Language::Typescript),
        "tsx" => Some(Language::Tsx),
        "java" => Some(Language::Java),
        "c" | "h" => Some(Language::C),
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => Some(Language::Cpp),
        "cs" => Some(Language::CSharp),
        "rb" | "rake" => Some(Language::Ruby),
        "php" => Some(Language::Php),
        "swift" => Some(Language::Swift),
        "kt" | "kts" => Some(Language::Kotlin),
        "md" | "markdown" => Some(Language::Markdown),
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
        Language::Java => tree_sitter_java::LANGUAGE.into(),
        Language::C => tree_sitter_c::LANGUAGE.into(),
        Language::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        Language::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
        Language::Ruby => tree_sitter_ruby::LANGUAGE.into(),
        Language::Php => tree_sitter_php::LANGUAGE_PHP.into(),
        Language::Swift => tree_sitter_swift::LANGUAGE.into(),
        Language::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
        Language::Markdown => tree_sitter_md::LANGUAGE.into(),
    }
}

/// Fence tag used when emitting a fenced code block for this language.
pub fn code_fence_tag(lang: Language) -> &'static str {
    match lang {
        Language::Rust => "rust",
        Language::Python => "python",
        Language::Go => "go",
        Language::Javascript => "javascript",
        Language::Typescript | Language::Tsx => "typescript",
        Language::Java => "java",
        Language::C => "c",
        Language::Cpp => "cpp",
        Language::CSharp => "csharp",
        Language::Ruby => "ruby",
        Language::Php => "php",
        Language::Swift => "swift",
        Language::Kotlin => "kotlin",
        Language::Markdown => "markdown",
    }
}

/// How to tell whether a declaration is part of a file's public surface.
/// Languages differ enough here that a single heuristic gets it wrong either
/// way, so each language names the convention it actually uses.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VisibilityRule {
    /// Public only with one of these leading keywords.
    Keyword(&'static [&'static str]),
    /// Public unless one of these leading keywords is present.
    UnlessKeyword(&'static [&'static str]),
    /// Public unless the name starts with an underscore.
    Underscore,
    /// Public when the name starts with a capital letter.
    Capitalized,
}

pub fn visibility_rule(lang: Language) -> VisibilityRule {
    match lang {
        Language::Rust => VisibilityRule::Keyword(&["pub"]),
        Language::Javascript | Language::Typescript | Language::Tsx => {
            VisibilityRule::Keyword(&["export"])
        }
        Language::Java | Language::CSharp | Language::Php => VisibilityRule::Keyword(&["public"]),
        Language::Swift => VisibilityRule::Keyword(&["public", "open"]),
        // Kotlin declarations are public by default.
        Language::Kotlin => VisibilityRule::UnlessKeyword(&["private", "internal", "protected"]),
        // C and C++ expose anything without internal linkage.
        Language::C | Language::Cpp => VisibilityRule::UnlessKeyword(&["static"]),
        Language::Go => VisibilityRule::Capitalized,
        Language::Python | Language::Ruby => VisibilityRule::Underscore,
        Language::Markdown => VisibilityRule::UnlessKeyword(&[]),
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

    #[test]
    fn test_infer_language_extended_extensions() {
        assert_eq!(
            infer_language(Path::new("bundle.mjs")),
            Some(Language::Javascript)
        );
        assert_eq!(
            infer_language(Path::new("types.d.mts")),
            Some(Language::Typescript)
        );
        assert_eq!(
            infer_language(Path::new("stubs.pyi")),
            Some(Language::Python)
        );
        assert_eq!(infer_language(Path::new("Main.java")), Some(Language::Java));
        assert_eq!(infer_language(Path::new("app.kt")), Some(Language::Kotlin));
        assert_eq!(
            infer_language(Path::new("view.swift")),
            Some(Language::Swift)
        );
    }

    #[test]
    fn test_every_language_has_a_grammar_and_fence() {
        for lang in Language::value_variants() {
            let _ = get_ts_language(*lang);
            assert!(!code_fence_tag(*lang).is_empty());
        }
    }
}

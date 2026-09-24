use crate::languages::{self, Language};
use crate::parser::{self, Symbol};
use crate::queries;
use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// One analyzed source file: its symbols, its imports, and enough metadata to
/// rank it and render it in any output format.
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub path: PathBuf,
    #[serde(serialize_with = "serialize_lang")]
    pub language: Language,
    pub line_count: usize,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<String>,
    /// Import-graph centrality, filled in by `rank`. 0.0 until then.
    #[serde(skip_serializing_if = "is_zero", serialize_with = "serialize_score")]
    pub score: f64,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// Rounded so that JSON output is byte-stable across platforms; the extra
/// precision carries no meaning for a ranking score anyway.
fn serialize_score<S: serde::Serializer>(score: &f64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_f64((score * 1e6).round() / 1e6)
}

fn serialize_lang<S: serde::Serializer>(lang: &Language, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(languages::code_fence_tag(*lang))
}

impl FileEntry {
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty() && self.imports.is_empty()
    }
}

/// Extracts every symbol in `path`.
///
/// `include_locals` keeps bindings declared inside function bodies. They are
/// filtered here rather than at render time so that every output format and
/// the token budget agree on what the file contains.
pub fn analyze_file(path: &Path, lang: Language, include_locals: bool) -> Result<FileEntry> {
    let content = fs::read_to_string(path)?;
    let line_count = content.lines().count();
    let ts_lang = languages::get_ts_language(lang);
    let q = queries::queries_for(lang);

    let Some(tree) = parser::parse_source(&content, &ts_lang) else {
        return Ok(FileEntry {
            path: path.to_path_buf(),
            language: lang,
            line_count,
            symbols: Vec::new(),
            imports: Vec::new(),
            score: 0.0,
        });
    };

    let mut symbols = parser::extract_symbols(
        &content,
        &tree,
        &ts_lang,
        q.symbols,
        languages::visibility_rule(lang),
    );
    if !include_locals {
        symbols.retain(|symbol| !symbol.local);
    }
    let imports = match q.imports {
        Some(query) => parser::extract_imports(&content, &tree, &ts_lang, query),
        None => Vec::new(),
    };

    Ok(FileEntry {
        path: path.to_path_buf(),
        language: lang,
        line_count,
        symbols,
        imports,
        score: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn analyze_str(contents: &str, lang: Language, ext: &str) -> FileEntry {
        let mut file = tempfile::Builder::new()
            .suffix(ext)
            .tempfile()
            .expect("temp file");
        file.write_all(contents.as_bytes()).expect("write");
        analyze_file(file.path(), lang, true).expect("analysis failed")
    }

    fn names(entry: &FileEntry) -> Vec<String> {
        entry.symbols.iter().map(|s| s.display_name()).collect()
    }

    #[test]
    fn test_markdown_headings() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "# Header 1\n## Header 2").expect("write");

        let entry = analyze_file(file.path(), Language::Markdown, true).expect("analysis failed");

        assert_eq!(entry.symbols.len(), 2);
        assert_eq!(entry.symbols[0].kind, "h1");
        assert_eq!(entry.symbols[1].kind, "h2");
        assert_eq!(entry.symbols[1].display_name(), "  Header 2");
    }

    #[test]
    fn test_python_decorated_method_keeps_its_class() {
        let entry = analyze_str(
            "class Foo:\n    @property\n    def baz(self):\n        return 1\n",
            Language::Python,
            ".py",
        );

        let baz = entry
            .symbols
            .iter()
            .find(|s| s.name == "baz")
            .expect("baz not found");
        assert_eq!(baz.parent.as_deref(), Some("Foo"));
        assert_eq!(baz.kind, "method");
    }

    #[test]
    fn test_python_decorated_method_is_not_duplicated() {
        let entry = analyze_str(
            "class Foo:\n    @staticmethod\n    def bar():\n        pass\n",
            Language::Python,
            ".py",
        );

        let bars = entry.symbols.iter().filter(|s| s.name == "bar").count();
        assert_eq!(bars, 1, "decorated method should appear once");
    }

    #[test]
    fn test_rust_covers_enum_trait_type_const_and_mod() {
        let entry = analyze_str(
            "pub enum Mode { A, B }\n\
             pub trait Render { fn render(&self) -> String; }\n\
             pub type Alias = u32;\n\
             pub const MAX: u32 = 10;\n\
             pub mod inner { pub fn nested() {} }\n",
            Language::Rust,
            ".rs",
        );

        let found = names(&entry);
        for expected in [
            "Mode",
            "Render",
            "Render > render",
            "Alias",
            "MAX",
            "inner",
            "inner > nested",
        ] {
            assert!(
                found.iter().any(|n| n == expected),
                "missing {expected:?} in {found:?}"
            );
        }
    }

    #[test]
    fn test_module_scoped_function_is_a_fn_not_a_method() {
        let entry = analyze_str(
            "pub mod inner { pub fn nested() {} }\n",
            Language::Rust,
            ".rs",
        );

        let nested = entry
            .symbols
            .iter()
            .find(|s| s.name == "nested")
            .expect("nested");
        assert_eq!(nested.display_name(), "inner > nested");
        assert_eq!(nested.kind, "fn", "a module is a namespace, not a receiver");
    }

    #[test]
    fn test_rust_signatures_and_visibility() {
        let entry = analyze_str(
            "pub fn visible(a: u32, b: &str) -> Result<()> { Ok(()) }\n\
             fn hidden() {}\n",
            Language::Rust,
            ".rs",
        );

        let visible = entry
            .symbols
            .iter()
            .find(|s| s.name == "visible")
            .expect("visible");
        assert_eq!(
            visible.signature.as_deref(),
            Some("pub fn visible(a: u32, b: &str) -> Result<()>")
        );
        assert!(visible.exported);

        let hidden = entry
            .symbols
            .iter()
            .find(|s| s.name == "hidden")
            .expect("hidden");
        assert!(!hidden.exported);
    }

    #[test]
    fn test_tsx_covers_arrow_consts_types_and_enums() {
        let entry = analyze_str(
            "import React from 'react';\n\
             export const Button = ({label}: {label: string}) => <button>{label}</button>;\n\
             export function Card() { return <div/>; }\n\
             export type Props = { id: number };\n\
             export enum Color { Red, Blue }\n\
             const helper = () => 42;\n",
            Language::Tsx,
            ".tsx",
        );

        let found = names(&entry);
        for expected in ["Button", "Card", "Props", "Color", "helper"] {
            assert!(
                found.iter().any(|n| n == expected),
                "missing {expected:?} in {found:?}"
            );
        }

        let button = entry
            .symbols
            .iter()
            .find(|s| s.name == "Button")
            .expect("Button");
        assert!(button.exported);
        assert!(
            button
                .signature
                .as_deref()
                .is_some_and(|s| s.contains("label: string")),
            "arrow signature should keep its params, got {:?}",
            button.signature
        );

        let helper = entry
            .symbols
            .iter()
            .find(|s| s.name == "helper")
            .expect("helper");
        assert!(
            !helper.exported,
            "non-exported const should not be exported"
        );
    }

    #[test]
    fn test_go_covers_interface_methods_and_receivers() {
        let entry = analyze_str(
            "package main\n\
             type Store interface {\n\tGet(id string) error\n}\n\
             type Impl struct{}\n\
             func (i *Impl) Get(id string) error { return nil }\n\
             const Limit = 10\n",
            Language::Go,
            ".go",
        );

        let found = names(&entry);
        for expected in ["Store", "Store > Get", "Impl", "Impl > Get", "Limit"] {
            assert!(
                found.iter().any(|n| n == expected),
                "missing {expected:?} in {found:?}"
            );
        }
    }

    #[test]
    fn test_java_class_and_methods() {
        let entry = analyze_str(
            "package app;\nimport java.util.List;\n\
             public class Service {\n\
             \tpublic String run(int n) { return \"\"; }\n\
             }\n\
             interface Port { void send(); }\n",
            Language::Java,
            ".java",
        );

        let found = names(&entry);
        for expected in ["Service", "Service > run", "Port", "Port > send"] {
            assert!(
                found.iter().any(|n| n == expected),
                "missing {expected:?} in {found:?}"
            );
        }
        assert!(entry.imports.contains(&"java.util.List".to_string()));
    }

    #[test]
    fn test_ruby_class_methods_and_requires() {
        let entry = analyze_str(
            "require 'json'\nrequire_relative 'helper'\n\
             class Widget\n  def render\n    1\n  end\nend\n",
            Language::Ruby,
            ".rb",
        );

        let found = names(&entry);
        assert!(found.iter().any(|n| n == "Widget"));
        assert!(found.iter().any(|n| n == "Widget > render"));
        assert!(entry.imports.contains(&"json".to_string()));
        assert!(entry.imports.contains(&"helper".to_string()));
    }

    #[test]
    fn test_c_functions_and_includes() {
        let entry = analyze_str(
            "#include <stdio.h>\n#include \"local.h\"\n\
             typedef struct Point { int x; } Point;\n\
             int add(int a, int b) { return a + b; }\n",
            Language::C,
            ".c",
        );

        let found = names(&entry);
        assert!(found.iter().any(|n| n == "add"), "got {found:?}");
        assert!(entry.imports.contains(&"stdio.h".to_string()));
        assert!(entry.imports.contains(&"local.h".to_string()));
    }

    #[test]
    fn test_exported_type_alias_signature_is_not_truncated() {
        // The `export` prefix lives on a parent node, so the signature start
        // moves back; the end has to be measured from the same origin.
        let entry = analyze_str(
            "export type Config = { verbose: boolean };\n",
            Language::Typescript,
            ".ts",
        );

        let config = entry
            .symbols
            .iter()
            .find(|s| s.name == "Config")
            .expect("Config");
        assert_eq!(
            config.signature.as_deref(),
            Some("export type Config = { verbose: boolean }")
        );
    }

    #[test]
    fn test_arrow_const_is_reported_as_a_function() {
        let entry = analyze_str(
            "export const run = (n: number): void => {};\nexport const limit = 5;\n",
            Language::Typescript,
            ".ts",
        );

        let run = entry.symbols.iter().find(|s| s.name == "run").expect("run");
        assert_eq!(run.kind, "fn");

        let limit = entry
            .symbols
            .iter()
            .find(|s| s.name == "limit")
            .expect("limit");
        assert_eq!(limit.kind, "var");
    }

    #[test]
    fn test_python_keeps_module_constants_but_not_every_assignment() {
        let entry = analyze_str(
            "MAX_SIZE = 10\nlogger = get_logger()\n",
            Language::Python,
            ".py",
        );

        let names = names(&entry);
        assert!(names.iter().any(|n| n == "MAX_SIZE"), "got {names:?}");
        assert!(
            !names.iter().any(|n| n == "logger"),
            "lowercase module assignments are noise, got {names:?}"
        );
    }

    #[test]
    fn test_markdown_heading_span_is_one_line() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "# Only heading\n\nBody text.").expect("write");

        let entry = analyze_file(file.path(), Language::Markdown, true).expect("analysis failed");

        assert_eq!(entry.symbols[0].span(), 1);
    }

    #[test]
    fn test_signatures_are_truncated_rather_than_unbounded() {
        let long_params = (0..80)
            .map(|i| format!("arg{i}: u32"))
            .collect::<Vec<_>>()
            .join(", ");
        let entry = analyze_str(
            &format!("pub fn wide({long_params}) {{}}\n"),
            Language::Rust,
            ".rs",
        );

        let sig = entry.symbols[0].signature.as_deref().expect("signature");
        assert!(sig.len() <= 210, "signature was {} chars", sig.len());
        assert!(sig.ends_with("..."));
    }

    fn analyze_without_locals(contents: &str, lang: Language, ext: &str) -> FileEntry {
        let mut file = tempfile::Builder::new()
            .suffix(ext)
            .tempfile()
            .expect("temp file");
        file.write_all(contents.as_bytes()).expect("write");
        analyze_file(file.path(), lang, false).expect("analysis failed")
    }

    const NESTED: &str = "export const run = (n: number): void => {\n\
         \tconst store = makeStore();\n\
         \tlet attempts = 0;\n\
         \tfor (let i = 0; i < 10; i++) {}\n\
         };\n";

    #[test]
    fn test_nested_bindings_are_captured_with_their_enclosing_function() {
        let entry = analyze_str(NESTED, Language::Typescript, ".ts");
        let found = names(&entry);

        assert!(found.iter().any(|n| n == "run > store"), "got {found:?}");
        assert!(found.iter().any(|n| n == "run > attempts"), "got {found:?}");
    }

    #[test]
    fn test_loop_counters_are_not_symbols() {
        let entry = analyze_str(NESTED, Language::Typescript, ".ts");

        assert!(
            !entry.symbols.iter().any(|s| s.name == "i"),
            "a for-loop counter should be skipped, got {:?}",
            names(&entry)
        );
    }

    #[test]
    fn test_no_locals_keeps_only_the_outward_surface() {
        let entry = analyze_without_locals(NESTED, Language::Typescript, ".ts");

        assert_eq!(names(&entry), vec!["run".to_string()]);
    }

    #[test]
    fn test_file_scope_bindings_are_not_local() {
        let entry = analyze_str(
            "export const API = 'x';\nconst internal = 2;\n",
            Language::Typescript,
            ".ts",
        );

        assert!(
            entry.symbols.iter().all(|s| !s.local),
            "file-scope bindings should survive --no-locals, got {:?}",
            entry.symbols
        );

        let kept = analyze_without_locals(
            "export const API = 'x';\nconst internal = 2;\n",
            Language::Typescript,
            ".ts",
        );
        assert_eq!(kept.symbols.len(), 2);
    }

    #[test]
    fn test_multi_declarator_binding_yields_distinct_symbols() {
        // `let a, b` is one statement with two declarators. Anchoring on the
        // statement rendered the same label twice.
        let entry = analyze_str(
            "function advance() {\n\tlet stopped, stoppedTokens;\n\tconst a = 1, b = 2;\n}\n",
            Language::Typescript,
            ".ts",
        );

        let signatures: Vec<&str> = entry
            .symbols
            .iter()
            .filter_map(|s| s.signature.as_deref())
            .collect();

        assert!(signatures.contains(&"let stopped"), "got {signatures:?}");
        assert!(
            signatures.contains(&"let stoppedTokens"),
            "got {signatures:?}"
        );
        assert!(signatures.contains(&"const a = 1"), "got {signatures:?}");
        assert!(signatures.contains(&"const b = 2"), "got {signatures:?}");
    }

    #[test]
    fn test_binding_signature_keeps_its_keyword_and_export() {
        let entry = analyze_str(
            "export const API = 'x';\nlet mutable = 1;\n",
            Language::Typescript,
            ".ts",
        );

        let sig = |name: &str| {
            entry
                .symbols
                .iter()
                .find(|s| s.name == name)
                .and_then(|s| s.signature.as_deref())
                .unwrap_or_else(|| panic!("{name} not found"))
        };

        assert_eq!(sig("API"), "export const API = 'x'");
        assert_eq!(sig("mutable"), "let mutable = 1");
    }

    #[test]
    fn test_nearest_enclosing_scope_wins() {
        let entry = analyze_str(
            "function outer() {\n\
             \tconst nested = () => {\n\
             \t\tconst deep = 1;\n\
             \t};\n\
             }\n",
            Language::Typescript,
            ".ts",
        );

        let found = names(&entry);
        assert!(
            found.iter().any(|n| n == "nested > deep"),
            "a binding should name its closest enclosing declaration, got {found:?}"
        );
    }

    #[test]
    fn test_swift_struct_and_enum_are_not_reported_as_classes() {
        // The Swift grammar parses struct, enum and class all as
        // `class_declaration`, so the keyword has to settle the kind.
        let entry = analyze_str(
            "public struct Point { public func norm() -> Double { return 0 } }\n\
             public enum Color { case red }\n\
             public protocol Drawable { func draw() }\n\
             public class View {}\n",
            Language::Swift,
            ".swift",
        );

        let kind_of = |name: &str| {
            entry
                .symbols
                .iter()
                .find(|s| s.name == name)
                .map(|s| s.kind.as_str())
                .unwrap_or_else(|| panic!("{name} not found in {:?}", names(&entry)))
        };

        assert_eq!(kind_of("Point"), "struct");
        assert_eq!(kind_of("Color"), "enum");
        assert_eq!(kind_of("Drawable"), "protocol");
        assert_eq!(kind_of("View"), "class");
        assert_eq!(kind_of("norm"), "method");
    }

    #[test]
    fn test_kotlin_members_get_their_class_and_drop_their_body() {
        let entry = analyze_str(
            "class Service(val name: String) {\n\
             \tfun run(n: Int): String = \"\"\n\
             }\n\
             object Registry { fun get() {} }\n\
             val LIMIT = 10\n",
            Language::Kotlin,
            ".kt",
        );

        let found = names(&entry);
        for expected in [
            "Service",
            "Service > run",
            "Registry",
            "Registry > get",
            "LIMIT",
        ] {
            assert!(
                found.iter().any(|n| n == expected),
                "missing {expected:?} in {found:?}"
            );
        }

        let run = entry.symbols.iter().find(|s| s.name == "run").expect("run");
        assert_eq!(
            run.signature.as_deref(),
            Some("fun run(n: Int): String"),
            "the function body should be cut off"
        );
    }

    #[test]
    fn test_csharp_members_and_property_kind() {
        let entry = analyze_str(
            "using System;\n\
             namespace App {\n\
             \tpublic interface IStore { string Get(int id); }\n\
             \tpublic class Store : IStore {\n\
             \t\tpublic string Name { get; set; }\n\
             \t\tpublic Store() {}\n\
             \t\tpublic string Get(int id) { return \"\"; }\n\
             \t}\n\
             }\n",
            Language::CSharp,
            ".cs",
        );

        let kind_of = |name: &str| {
            entry
                .symbols
                .iter()
                .find(|s| s.name == name)
                .map(|s| s.kind.as_str())
                .unwrap_or_else(|| panic!("{name} not found in {:?}", names(&entry)))
        };

        assert_eq!(kind_of("App"), "module");
        assert_eq!(kind_of("IStore"), "interface");
        assert_eq!(kind_of("Name"), "property");
        assert_eq!(kind_of("Store"), "class");
        assert!(entry.imports.contains(&"System".to_string()));

        let get = entry
            .symbols
            .iter()
            .filter(|s| s.name == "Get")
            .collect::<Vec<_>>();
        assert_eq!(get.len(), 2, "one on the interface, one on the class");
        assert!(get.iter().all(|s| s.kind == "method"));
    }

    #[test]
    fn test_php_classes_traits_and_interfaces() {
        let entry = analyze_str(
            "<?php\n\
             use App\\Contracts\\Store;\n\
             interface Repo { public function find(int $id); }\n\
             trait Loggable { public function log(string $m) {} }\n\
             class UserRepo implements Repo {\n\
             \tpublic function find(int $id) { return null; }\n\
             }\n\
             function helper() {}\n",
            Language::Php,
            ".php",
        );

        let found = names(&entry);
        for expected in [
            "Repo",
            "Repo > find",
            "Loggable",
            "Loggable > log",
            "UserRepo",
            "UserRepo > find",
            "helper",
        ] {
            assert!(
                found.iter().any(|n| n == expected),
                "missing {expected:?} in {found:?}"
            );
        }
    }

    #[test]
    fn test_cpp_member_prototype_is_a_method_not_a_field() {
        let entry = analyze_str(
            "#include <vector>\n\
             class Widget {\n\
             public:\n\
             \tvoid render();\n\
             \tint size() const { return 0; }\n\
             \tint count;\n\
             };\n",
            Language::Cpp,
            ".cpp",
        );

        let kind_of = |name: &str| {
            entry
                .symbols
                .iter()
                .find(|s| s.name == name)
                .map(|s| s.kind.as_str())
                .unwrap_or_else(|| panic!("{name} not found in {:?}", names(&entry)))
        };

        assert_eq!(kind_of("Widget"), "class");
        assert_eq!(
            kind_of("render"),
            "method",
            "a prototype is a function, not a data member"
        );
        assert_eq!(kind_of("size"), "method");
        assert!(entry.imports.contains(&"vector".to_string()));
    }

    #[test]
    fn test_container_precedes_its_members_on_a_shared_line() {
        let entry = analyze_str(
            "public interface IStore { string Get(int id); }\n",
            Language::CSharp,
            ".cs",
        );

        assert_eq!(
            names(&entry),
            vec!["IStore".to_string(), "IStore > Get".to_string()]
        );
    }

    #[test]
    fn test_unparseable_file_yields_no_symbols_but_counts_lines() {
        let entry = analyze_str("!!!!not rust at all((((", Language::Rust, ".rs");
        assert_eq!(entry.line_count, 1);
    }
}

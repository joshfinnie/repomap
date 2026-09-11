use std::collections::HashSet;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator, Tree};

pub struct Symbol {
    pub name: String,
    pub parent: Option<String>,
    pub line: usize,
    pub kind: String,
    pub end_line: usize,
}

/// Parses `source` once so the resulting tree can be reused across
/// both symbol and import extraction, instead of parsing twice per file.
pub fn parse_source(source: &str, lang: &tree_sitter::Language) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(lang).ok()?;
    parser.parse(source, None)
}

pub fn extract_symbols(
    source: &str,
    tree: &Tree,
    lang: &tree_sitter::Language,
    query_str: &str,
) -> Vec<Symbol> {
    let query = Query::new(lang, query_str).expect("Failed to create query");
    let mut cursor = QueryCursor::new();

    let mut symbols = Vec::new();
    let mut parented_lines: HashSet<usize> = HashSet::new();
    let source_bytes = source.as_bytes();

    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);

    while let Some(m) = matches.next() {
        let mut name = String::new();
        let mut parent = None;
        let mut kind = String::new();
        let mut start_line = 0;
        let mut end_line = 0;

        for capture in m.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            let node = capture.node;

            match capture_name {
                "name" => {
                    if let Some(n) = source.get(node.start_byte()..node.end_byte()) {
                        name = n.trim().to_string();
                    }
                }
                "parent" => {
                    if let Some(p) = source.get(node.start_byte()..node.end_byte()) {
                        parent = Some(p.to_string());
                    }
                }
                "item" => {
                    let node_kind = node.kind();
                    kind = node_kind.to_string();
                    start_line = node.start_position().row + 1;
                    end_line = node.end_position().row + 1;

                    if node_kind == "atx_heading"
                        && let Some(raw_text) = source.get(node.start_byte()..node.end_byte())
                    {
                        let level = raw_text.chars().take_while(|&c| c == '#').count();
                        kind = format!("h{}", level);
                        name = raw_text.trim_start_matches('#').trim().to_string();
                    }
                }
                _ => {}
            }
        }

        if !name.is_empty() && start_line > 0 {
            let is_duplicate = parent.is_none() && parented_lines.contains(&start_line);
            if !is_duplicate {
                if parent.is_some() {
                    parented_lines.insert(start_line);
                }
                symbols.push(Symbol {
                    name,
                    kind,
                    parent,
                    line: start_line,
                    end_line,
                });
            }
        }
    }

    symbols
}

pub fn extract_imports(
    source: &str,
    tree: &Tree,
    lang: &tree_sitter::Language,
    query_str: &str,
) -> Vec<String> {
    let query = match Query::new(lang, query_str) {
        Ok(q) => q,
        Err(_) => return vec![],
    };

    let mut cursor = QueryCursor::new();
    let source_bytes = source.as_bytes();
    let mut imports = Vec::new();

    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);

    while let Some(m) = matches.next() {
        for capture in m.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            if capture_name == "import"
                && let Some(text) = source.get(capture.node.start_byte()..capture.node.end_byte())
            {
                // Clean up the import string (remove quotes, trim)
                let cleaned = text.trim().trim_matches('"').trim_matches('\'').to_string();
                if !cleaned.is_empty() && !imports.contains(&cleaned) {
                    imports.push(cleaned);
                }
            }
        }
    }

    imports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_symbol_extraction() {
        let code = "struct MyStruct { field: i32 } fn my_func() {}";
        let lang = tree_sitter_rust::LANGUAGE.into();
        let query = "(function_item name: (identifier) @name) @item (struct_item name: (type_identifier) @name) @item";
        let tree = parse_source(code, &lang).unwrap();

        let symbols = extract_symbols(code, &tree, &lang, query);

        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "MyStruct");
        assert_eq!(symbols[1].name, "my_func");
    }

    #[test]
    fn test_rust_import_extraction() {
        let code = "use std::path::Path;\nuse crate::parser;\nfn main() {}";
        let lang = tree_sitter_rust::LANGUAGE.into();
        let query = "(use_declaration argument: (_) @import)";
        let tree = parse_source(code, &lang).unwrap();

        let imports = extract_imports(code, &tree, &lang, query);

        assert_eq!(imports.len(), 2);
        assert!(imports.contains(&"std::path::Path".to_string()));
        assert!(imports.contains(&"crate::parser".to_string()));
    }

    #[test]
    fn test_typescript_import_extraction() {
        let code = "import { foo } from './foo';\nimport React from 'react';";
        let lang = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let query = "(import_statement source: (string) @import)";
        let tree = parse_source(code, &lang).unwrap();

        let imports = extract_imports(code, &tree, &lang, query);

        assert_eq!(imports.len(), 2);
        assert!(imports.contains(&"./foo".to_string()));
        assert!(imports.contains(&"react".to_string()));
    }

    #[test]
    fn test_extract_symbols_dedup_is_linear() {
        // A method line should only be kept once, preferring the parented match.
        let code = "impl Foo { fn bar() {} }";
        let lang = tree_sitter_rust::LANGUAGE.into();
        let query = "(function_item name: (identifier) @name) @item
             (impl_item
                type: (_) @parent
                body: (declaration_list (function_item name: (identifier) @name) @item))";
        let tree = parse_source(code, &lang).unwrap();

        let symbols = extract_symbols(code, &tree, &lang, query);

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].parent.as_deref(), Some("Foo"));
    }
}

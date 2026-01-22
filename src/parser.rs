use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

pub struct Symbol {
    pub name: String,
    pub parent: Option<String>,
    pub line: usize,
    pub kind: String,
    pub end_line: usize,
}

pub fn extract_symbols(source: &str, lang: &tree_sitter::Language, query_str: &str) -> Vec<Symbol> {
    let mut parser = Parser::new();
    parser.set_language(lang).expect("Error loading grammar");

    let tree = parser.parse(source, None).expect("Failed to parse source");
    let query = Query::new(lang, query_str).expect("Failed to create query");
    let mut cursor = QueryCursor::new();

    let mut symbols = Vec::new();
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
            let is_duplicate = symbols
                .iter()
                .any(|s: &Symbol| s.line == start_line && s.parent.is_some() && parent.is_none());
            if !is_duplicate {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_symbol_extraction() {
        let code = "struct MyStruct { field: i32 } fn my_func() {}";
        let lang = tree_sitter_rust::LANGUAGE.into();
        let query = "(function_item name: (identifier) @name) @item (struct_item name: (type_identifier) @name) @item";

        let symbols = extract_symbols(code, &lang, query);

        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "MyStruct");
        assert_eq!(symbols[1].name, "my_func");
    }
}

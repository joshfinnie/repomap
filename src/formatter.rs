use crate::languages::{self, Language};
use crate::parser;
use anyhow::Result;
use std::fs;
use std::path::Path;

pub struct RepoStats {
    pub map_content: String,
    pub table_rows: String,
    pub file_count: usize,
}

impl RepoStats {
    pub fn new() -> Self {
        Self {
            map_content: String::new(),
            table_rows: String::new(),
            file_count: 0,
        }
    }

    pub fn add_file(&mut self, path: &Path, file_map: String, sym_count: usize, line_count: usize) {
        self.map_content.push_str(&file_map);
        self.table_rows.push_str(&format!(
            "| `{}` | {} | {} |\n",
            path.display(),
            sym_count,
            line_count
        ));
        self.file_count += 1;
    }

    pub fn estimate_tokens(&self, final_output: &str) -> usize {
        final_output.len() / 4
    }
}

fn get_import_query(lang: Language) -> Option<&'static str> {
    match lang {
        Language::Rust => Some(
            "(use_declaration argument: (_) @import)",
        ),
        Language::Python => Some(
            "(import_statement name: (dotted_name) @import)
             (import_from_statement module_name: (dotted_name) @import)
             (import_from_statement module_name: (relative_import) @import)",
        ),
        Language::Go => Some(
            "(import_spec path: (interpreted_string_literal) @import)",
        ),
        Language::Javascript | Language::Typescript | Language::Tsx => Some(
            "(import_statement source: (string) @import)
             (export_statement source: (string) @import)",
        ),
        Language::Markdown => None,
    }
}

pub fn process_file_with_stats(path: &Path, lang: Language) -> Result<(String, usize, usize)> {
    let content = fs::read_to_string(path)?;
    let ts_lang = languages::get_ts_language(lang);

    let (query_str, lang_tag) = match lang {
        Language::Rust => (
            "(function_item name: (identifier) @name) @item
             (struct_item name: (type_identifier) @name) @item
             (impl_item
                type: (_) @parent
                body: (declaration_list (function_item name: (identifier) @name) @item))",
            "rust",
        ),
        Language::Python => (
            "(function_definition name: (identifier) @name) @item
             (class_definition name: (identifier) @name) @item
             (class_definition
                name: (identifier) @parent
                body: (block (function_definition name: (identifier) @name) @item))",
            "python",
        ),
        Language::Go => (
            "(function_declaration name: (identifier) @name) @item
             (type_spec name: (type_identifier) @name) @item
             (method_declaration
                receiver: (parameter_list (parameter_declaration type: (_) @parent))
                name: (field_identifier) @name) @item",
            "go",
        ),
        Language::Javascript => (
            "(function_declaration name: (identifier) @name) @item
             (class_declaration name: (identifier) @name) @item
             (class_declaration
                name: (identifier) @parent
                body: (class_body (method_definition name: (property_identifier) @name) @item))",
            "javascript",
        ),
        Language::Typescript | Language::Tsx => (
            "(function_declaration name: (identifier) @name) @item
             (class_declaration name: (type_identifier) @name) @item
             (interface_declaration name: (type_identifier) @name) @item
             (class_declaration
                name: (type_identifier) @parent
                body: (class_body (method_definition name: (property_identifier) @name) @item))",
            "typescript",
        ),
        Language::Markdown => ("(atx_heading) @item", "markdown"),
    };

    let symbols = parser::extract_symbols(&content, &ts_lang, query_str);

    // Extract imports
    let imports = if let Some(import_query) = get_import_query(lang) {
        parser::extract_imports(&content, &ts_lang, import_query)
    } else {
        vec![]
    };

    let mut file_output = String::new();

    if !symbols.is_empty() || !imports.is_empty() {
        file_output.push_str(&format!("\n## {}\n", path.display()));

        // Show imports first if present
        if !imports.is_empty() {
            file_output.push_str(&format!("imports: {}\n", imports.join(", ")));
        }

        if !symbols.is_empty() {
            file_output.push_str(&format!("```{}\n", lang_tag));
            for sym in &symbols {
                let size = sym.end_line - sym.line + 1;
                let display_name = match &sym.parent {
                    Some(p) => format!("{} > {}", p, sym.name),
                    None => {
                        if sym.kind.starts_with('h') && sym.kind.len() > 1 {
                            let level = sym.kind[1..].parse::<usize>().unwrap_or(1);
                            format!("{}{}", "  ".repeat(level.saturating_sub(1)), sym.name)
                        } else {
                            sym.name.clone()
                        }
                    }
                };
                file_output.push_str(&format!(
                    "L{: <3} | {: <10} | {: <30} | ({} lines)\n",
                    sym.line, sym.kind, display_name, size
                ));
            }
            file_output.push_str("```\n");
        }
    }

    Ok((file_output, symbols.len(), content.lines().count()))
}

pub fn assemble_final_map(root: &str, stats: &RepoStats, show_summary: bool) -> String {
    let mut output = format!(
        "# Repository Map\n**Root:** `{}`\n**Files:** {}\n\n",
        root, stats.file_count
    );
    if show_summary {
        output.push_str("## Summary\n| File | Symbols | Lines |\n| :--- | :--- | :--- |\n");
        output.push_str(&stats.table_rows);
        output.push_str("\n---\n");
    } else {
        output.push_str("---\n");
    }
    output.push_str(&stats.map_content);
    output
}

const REPOMAP_START: &str = "<!-- REPOMAP START -->";
const REPOMAP_END: &str = "<!-- REPOMAP END -->";

pub fn wrap_for_claude_md(content: &str, file_count: usize, token_estimate: usize) -> String {
    let token_display = if token_estimate >= 1000 {
        format!("~{:.1}k tokens", token_estimate as f64 / 1000.0)
    } else {
        format!("~{} tokens", token_estimate)
    };

    format!(
        "{}\n## Repository Structure\n\n<details>\n<summary>Repository map ({} files, {})</summary>\n\n{}\n</details>\n{}\n",
        REPOMAP_START, file_count, token_display, content, REPOMAP_END
    )
}

pub fn update_or_append_repomap(existing_content: &str, new_repomap: &str) -> String {
    if let (Some(start), Some(end)) = (
        existing_content.find(REPOMAP_START),
        existing_content.find(REPOMAP_END),
    ) {
        // Replace existing repomap section
        let before = &existing_content[..start];
        let after = &existing_content[end + REPOMAP_END.len()..];
        format!("{}{}{}", before.trim_end(), new_repomap, after)
    } else {
        // Append to existing content
        let trimmed = existing_content.trim_end();
        if trimmed.is_empty() {
            new_repomap.to_string()
        } else {
            format!("{}\n\n{}", trimmed, new_repomap)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_markdown_formatting_logic() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(file, "# Header 1\n## Header 2").expect("Failed to write to temp file");

        let (output, sym_count, line_count) =
            process_file_with_stats(file.path(), Language::Markdown).expect("Processing failed");

        assert_eq!(sym_count, 2);
        assert_eq!(line_count, 2);
        assert!(output.contains("h1         | Header 1"));
        assert!(output.contains("h2         |   Header 2"));
    }

    #[test]
    fn test_repostats_aggregation() {
        let mut stats = RepoStats::new();
        let path = Path::new("src/main.rs");

        stats.add_file(path, "Dummy content".to_string(), 5, 100);

        assert_eq!(stats.file_count, 1);
        assert!(stats.table_rows.contains("| `src/main.rs` | 5 | 100 |"));
    }

    #[test]
    fn test_token_estimation() {
        let stats = RepoStats::new();
        let dummy_output = "a".repeat(400);
        assert_eq!(stats.estimate_tokens(&dummy_output), 100);
    }
}

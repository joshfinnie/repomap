use crate::analyze::FileEntry;
use crate::languages;
use anyhow::Result;
use serde::Serialize;

const REPOMAP_START: &str = "<!-- REPOMAP START -->";
const REPOMAP_END: &str = "<!-- REPOMAP END -->";

#[derive(Copy, Clone, PartialEq, Eq, clap::ValueEnum, Debug, Default)]
pub enum Format {
    #[default]
    Markdown,
    Json,
}

#[derive(Copy, Clone, Debug)]
pub struct RenderOptions {
    /// Symbol names and hierarchy only: no imports, line numbers, or fences.
    pub minimal: bool,
    /// Show full declaration signatures rather than bare names.
    pub signatures: bool,
    /// Prepend the per-file summary table.
    pub summary: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            minimal: false,
            signatures: true,
            summary: false,
        }
    }
}

/// Rough proxy for tokens. Deliberately cheap: it only has to be consistent
/// enough to compare files against a budget.
pub fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

pub fn render_entry(entry: &FileEntry, opts: &RenderOptions) -> String {
    let mut out = String::new();
    if entry.is_empty() {
        return out;
    }

    out.push_str(&format!("\n## {}\n", entry.path.display()));

    if !opts.minimal && !entry.imports.is_empty() {
        out.push_str(&format!("imports: {}\n", entry.imports.join(", ")));
    }

    if entry.symbols.is_empty() {
        return out;
    }

    // In minimal mode signatures are exactly the bulk we are trying to shed.
    let with_signature = opts.signatures && !opts.minimal;

    if opts.minimal {
        for sym in &entry.symbols {
            out.push_str(&format!("- {}\n", sym.label(false)));
        }
        return out;
    }

    out.push_str(&format!(
        "```{}\n",
        languages::code_fence_tag(entry.language)
    ));
    for sym in &entry.symbols {
        out.push_str(&format!(
            "L{: <4} | {: <9} | {: <60} | ({} lines)\n",
            sym.line,
            sym.kind,
            sym.label(with_signature),
            sym.span()
        ));
    }
    out.push_str("```\n");

    out
}

fn summary_table(entries: &[FileEntry]) -> String {
    let mut out = String::from("## Summary\n| File | Symbols | Lines |\n| :--- | :--- | :--- |\n");
    for entry in entries {
        out.push_str(&format!(
            "| `{}` | {} | {} |\n",
            entry.path.display(),
            entry.symbols.len(),
            entry.line_count
        ));
    }
    out.push_str("\n---\n");
    out
}

pub fn render_markdown(
    root: &str,
    entries: &[FileEntry],
    dropped: usize,
    opts: &RenderOptions,
) -> String {
    let mut out = format!(
        "# Repository Map\n**Root:** `{}`\n**Files:** {}\n",
        root,
        entries.len()
    );

    if dropped > 0 {
        out.push_str(&format!(
            "**Omitted:** {} lower-ranked file(s) to fit the token budget\n",
            dropped
        ));
    }
    out.push('\n');

    if opts.summary {
        out.push_str(&summary_table(entries));
    } else {
        out.push_str("---\n");
    }

    for entry in entries {
        out.push_str(&render_entry(entry, opts));
    }

    out
}

#[derive(Serialize)]
struct JsonMap<'a> {
    root: &'a str,
    file_count: usize,
    token_estimate: usize,
    #[serde(skip_serializing_if = "is_zero_usize")]
    omitted_files: usize,
    files: &'a [FileEntry],
}

fn is_zero_usize(v: &usize) -> bool {
    *v == 0
}

pub fn render_json(
    root: &str,
    entries: &[FileEntry],
    dropped: usize,
    token_estimate: usize,
) -> Result<String> {
    let map = JsonMap {
        root,
        file_count: entries.len(),
        token_estimate,
        omitted_files: dropped,
        files: entries,
    };

    Ok(serde_json::to_string_pretty(&map)?)
}

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
        let before = &existing_content[..start];
        let after = &existing_content[end + REPOMAP_END.len()..];
        format!("{}{}{}", before.trim_end(), new_repomap, after)
    } else {
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
    use crate::languages::Language;
    use crate::parser::Symbol;
    use std::path::PathBuf;

    fn sample() -> FileEntry {
        FileEntry {
            path: PathBuf::from("src/lib.rs"),
            language: Language::Rust,
            line_count: 40,
            imports: vec!["std::fs".to_string()],
            score: 0.0,
            symbols: vec![
                Symbol {
                    name: "Store".to_string(),
                    parent: None,
                    line: 3,
                    end_line: 9,
                    kind: "struct".to_string(),
                    signature: Some("pub struct Store".to_string()),
                    exported: true,
                },
                Symbol {
                    name: "get".to_string(),
                    parent: Some("Store".to_string()),
                    line: 12,
                    end_line: 15,
                    kind: "method".to_string(),
                    signature: Some("fn get(&self, id: u32) -> Option<&str>".to_string()),
                    exported: false,
                },
            ],
        }
    }

    #[test]
    fn test_markdown_shows_signatures_and_breadcrumbs() {
        let out = render_entry(&sample(), &RenderOptions::default());

        assert!(out.contains("imports: std::fs"));
        assert!(out.contains("```rust"));
        assert!(out.contains("pub struct Store"));
        assert!(
            out.contains("Store > fn get(&self, id: u32) -> Option<&str>"),
            "got {out}"
        );
        assert!(out.contains("(4 lines)"));
    }

    #[test]
    fn test_signatures_can_be_disabled() {
        let opts = RenderOptions {
            signatures: false,
            ..RenderOptions::default()
        };
        let out = render_entry(&sample(), &opts);

        assert!(!out.contains("pub struct Store"));
        assert!(out.contains("| Store "));
        assert!(out.contains("Store > get"));
    }

    #[test]
    fn test_minimal_omits_imports_signatures_and_fences() {
        let opts = RenderOptions {
            minimal: true,
            ..RenderOptions::default()
        };
        let out = render_entry(&sample(), &opts);

        assert!(!out.contains("imports:"));
        assert!(!out.contains("```"));
        assert!(!out.contains(" | "));
        assert!(!out.contains("pub struct"));
        assert!(out.contains("- Store\n"));
        assert!(out.contains("- Store > get\n"));
    }

    #[test]
    fn test_markdown_reports_omitted_files() {
        let entries = vec![sample()];
        let out = render_markdown(".", &entries, 7, &RenderOptions::default());

        assert!(
            out.contains("**Omitted:** 7 lower-ranked file(s)"),
            "got {out}"
        );
    }

    #[test]
    fn test_markdown_omits_the_note_when_nothing_dropped() {
        let entries = vec![sample()];
        let out = render_markdown(".", &entries, 0, &RenderOptions::default());

        assert!(!out.contains("Omitted"));
    }

    #[test]
    fn test_json_round_trips_symbols() {
        let entries = vec![sample()];
        let json = render_json(".", &entries, 0, 99).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

        assert_eq!(parsed["file_count"], 1);
        assert_eq!(parsed["token_estimate"], 99);
        assert_eq!(parsed["files"][0]["path"], "src/lib.rs");
        assert_eq!(parsed["files"][0]["language"], "rust");
        assert_eq!(parsed["files"][0]["symbols"][0]["name"], "Store");
        assert_eq!(parsed["files"][0]["symbols"][0]["exported"], true);
        assert_eq!(parsed["files"][0]["symbols"][1]["parent"], "Store");
        assert!(parsed["files"][0].get("omitted_files").is_none());
    }

    #[test]
    fn test_summary_table_is_included_on_request() {
        let opts = RenderOptions {
            summary: true,
            ..RenderOptions::default()
        };
        let out = render_markdown(".", &[sample()], 0, &opts);

        assert!(out.contains("| `src/lib.rs` | 2 | 40 |"));
    }

    #[test]
    fn test_repomap_section_is_replaced_in_place() {
        let existing = format!(
            "# Project\n\nSome notes.\n\n{}\nold map\n{}\n\nTrailing notes.\n",
            REPOMAP_START, REPOMAP_END
        );
        let updated = update_or_append_repomap(&existing, "NEW");

        assert!(updated.contains("Some notes."));
        assert!(updated.contains("Trailing notes."));
        assert!(updated.contains("NEW"));
        assert!(!updated.contains("old map"));
    }

    #[test]
    fn test_repomap_is_appended_when_absent() {
        let updated = update_or_append_repomap("# Project\n", "NEW");
        assert!(updated.starts_with("# Project"));
        assert!(updated.contains("NEW"));
    }

    #[test]
    fn test_token_estimation() {
        assert_eq!(estimate_tokens(&"a".repeat(400)), 100);
    }
}

mod parser;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use ignore::WalkBuilder;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(author, version, about = "Generate a repository map for AI context")]
struct Args {
    #[arg(
        short,
        long,
        help = "Force a specific language parser (overrides auto-detection)"
    )]
    language: Option<Language>,

    #[arg(default_value = ".")]
    root: String,

    #[arg(short, long)]
    output: Option<PathBuf>,

    #[arg(short, long)]
    exclude: Vec<String>,

    #[arg(short, long)]
    depth: Option<usize>,

    #[arg(short, long)]
    summary: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum Language {
    Rust,
    Python,
    Go,
    Javascript,
    Typescript,
    Tsx,
    Markdown,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut map_content = String::new();
    let mut file_count = 0;

    let mut table_rows = String::from("| File | Symbols | Lines |\n| :--- | :--- | :--- |\n");

    let mut walker = WalkBuilder::new(&args.root);

    if let Some(d) = args.depth {
        walker.max_depth(Some(d));
    }

    for pattern in &args.exclude {
        walker.add_custom_ignore_filename(pattern);
    }

    for result in walker.build() {
        let entry = result?;
        let path = entry.path();

        if path.is_file() {
            let target_lang = args.language.or_else(|| infer_language(path));

            if let Some(lang) = target_lang
                && let Ok((file_map, sym_count, line_count)) = process_file(path, lang)
                && !file_map.is_empty()
            {
                map_content.push_str(&file_map);
                table_rows.push_str(&format!(
                    "| `{}` | {} | {} |\n",
                    path.display(),
                    sym_count,
                    line_count
                ));
                file_count += 1;
            }
        }
    }

    let header = format!(
        "# Repository Map\n**Root:** `{}`\n**Files Processed:** {}\n\n---\n",
        args.root, file_count
    );

    let mut final_output = header;

    if args.summary {
        final_output.push_str("## Summary\n");
        final_output.push_str(&table_rows);
        final_output.push_str("\n---\n");
    } else {
        final_output.push_str("---\n");
    }

    final_output.push_str(&map_content);

    let token_est = estimate_tokens(&final_output);
    eprintln!("----------------------------------------");
    eprintln!("Processed {} files.", file_count);
    eprintln!("Estimated Tokens: ~{}", token_est);
    eprintln!("----------------------------------------");

    if let Some(output_path) = args.output {
        std::fs::write(&output_path, &final_output)?;
        println!("Map successfully written to: {}", output_path.display());
    } else {
        println!("{}", final_output);
    }

    Ok(())
}

fn infer_language(path: &Path) -> Option<Language> {
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

fn process_file(path: &Path, lang: Language) -> Result<(String, usize, usize)> {
    let content = fs::read_to_string(path)?;
    let mut output = String::new();
    let total_lines_in_file = content.lines().count();

    let ts_lang: tree_sitter::Language = match lang {
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::Go => tree_sitter_go::LANGUAGE.into(),
        Language::Javascript => tree_sitter_javascript::LANGUAGE.into(),
        Language::Typescript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Language::Markdown => tree_sitter_md::LANGUAGE.into(),
    };

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
        Language::Javascript | Language::Typescript | Language::Tsx => (
            "(function_declaration name: (identifier) @name) @item
             (class_declaration name: (identifier) @name) @item
             (interface_declaration name: (type_identifier) @name) @item
             (class_declaration 
                name: (type_identifier) @parent 
                body: (class_body (method_definition name: (property_identifier) @name) @item))",
            "typescript",
        ),
        Language::Markdown => ("(atx_heading) @item", "markdown"),
    };

    let symbols = parser::extract_symbols(&content, &ts_lang, query_str);

    if !symbols.is_empty() {
        output.push_str(&format!("\n## {}\n", path.display()));
        output.push_str(&format!("```{lang_tag}\n"));
        for sym in &symbols {
            let size = sym.end_line - sym.line + 1;

            let clean_kind = sym
                .kind
                .replace("_item", "")
                .replace("_definition", "")
                .replace("_declaration", "");

            let display_name = match &sym.parent {
                Some(p) => format!("{} > {}", p, sym.name),
                None => {
                    if sym.kind.starts_with('h') && sym.kind.len() > 1 {
                        let level = sym.kind[1..].parse::<usize>().unwrap_or(1);
                        let indent = "  ".repeat(level.saturating_sub(1));
                        format!("{}{}", indent, sym.name)
                    } else {
                        sym.name.clone()
                    }
                }
            };

            output.push_str(&format!(
                "L{: <3} | {: <10} | {: <30} | ({} lines)\n",
                sym.line, clean_kind, display_name, size
            ));
        }
        output.push_str("```\n");
    }
    Ok((output, symbols.len(), total_lines_in_file))
}

fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

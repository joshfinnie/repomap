mod analyze;
mod git;
mod languages;
mod parser;
mod queries;
mod rank;
mod render;
mod walk;

use anyhow::{Context, Result};
use clap::Parser;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

use languages::Language;
use render::{Format, RenderOptions};

/// Files repomap itself generates, always skipped to avoid feeding the map
/// back into itself.
const EXCLUDED_FILES: &[&str] = &["repomap.md", "CLAUDE.md"];

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

    #[arg(short, long, default_missing_value = "repomap.md", num_args = 0..=1)]
    output: Option<PathBuf>,

    #[arg(short, long)]
    exclude: Vec<String>,

    #[arg(short, long)]
    depth: Option<usize>,

    #[arg(short, long)]
    summary: bool,

    #[arg(
        short,
        long,
        help = "Emit only symbol names and hierarchy, omitting imports, line numbers, and code blocks"
    )]
    minimal: bool,

    #[arg(
        long,
        help = "Print bare symbol names instead of full declaration signatures"
    )]
    no_signatures: bool,

    #[arg(short, long, value_enum, default_value_t = Format::Markdown, help = "Output format")]
    format: Format,

    #[arg(
        long,
        value_name = "TOKENS",
        help = "Keep only the highest-ranked files that fit this token budget"
    )]
    max_tokens: Option<usize>,

    #[arg(
        long,
        value_name = "PATH",
        help = "Rank files by import-graph proximity to this path instead of by global centrality"
    )]
    focus: Option<PathBuf>,

    #[arg(
        long,
        value_name = "REF",
        help = "Map only files changed since this git ref (e.g. HEAD~1, main)"
    )]
    since: Option<String>,

    #[arg(
        long,
        help = "Output to CLAUDE.md with smart update (append or replace)"
    )]
    claude: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let opts = RenderOptions {
        minimal: args.minimal,
        signatures: !args.no_signatures,
        summary: args.summary,
    };

    let output_path = resolve_output_path(&args);
    let output_canonical = output_path.as_ref().and_then(|p| p.canonicalize().ok());

    let changed = match &args.since {
        Some(git_ref) => Some(
            git::changed_files(&args.root, git_ref)
                .with_context(|| format!("could not list files changed since {git_ref}"))?,
        ),
        None => None,
    };

    // Walking and language inference are cheap and I/O bound; collect
    // candidates first so the CPU-heavy parsing below runs across files in
    // parallel.
    let mut candidates: Vec<(PathBuf, Language)> = Vec::new();
    for result in walk::create_walker(&args.root, args.depth, &args.exclude)? {
        let entry = result?;
        let path = entry.path();

        if !path.is_file() || walk::is_binary(path) {
            continue;
        }
        if is_generated_output(path, output_canonical.as_deref()) {
            continue;
        }
        if let Some(changed) = &changed
            && !git::is_changed(changed, path)
        {
            continue;
        }

        if let Some(lang) = args.language.or_else(|| languages::infer_language(path)) {
            candidates.push((path.to_path_buf(), lang));
        }
    }

    let mut entries: Vec<analyze::FileEntry> = candidates
        .par_iter()
        .filter_map(|(path, lang)| {
            let entry = analyze::analyze_file(path, *lang).ok()?;
            (!entry.is_empty()).then_some(entry)
        })
        .collect();

    // Parsing order across threads is nondeterministic; sort by path so
    // output is stable and matches what a sequential walk would produce.
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    // Ranking needs the whole graph, so it happens after every file is parsed
    // and before any budget is applied.
    rank::score_entries(&mut entries, args.focus.as_deref());

    let (entries, dropped) = match args.max_tokens {
        Some(budget) => rank::apply_budget(entries, budget, |entry| {
            render::estimate_tokens(&render::render_entry(entry, &opts))
        }),
        None => (entries, 0),
    };

    let markdown = render::render_markdown(&args.root, &entries, dropped, &opts);
    let token_estimate = render::estimate_tokens(&markdown);

    let final_output = match args.format {
        Format::Markdown => markdown,
        Format::Json => render::render_json(&args.root, &entries, dropped, token_estimate)?,
    };

    report_progress(entries.len(), dropped, token_estimate);
    write_output(&args, &final_output, entries.len(), token_estimate)
}

fn resolve_output_path(args: &Args) -> Option<PathBuf> {
    if args.claude {
        Some(
            args.output
                .clone()
                .unwrap_or_else(|| PathBuf::from("CLAUDE.md")),
        )
    } else {
        args.output.clone()
    }
}

fn is_generated_output(path: &Path, output_canonical: Option<&Path>) -> bool {
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str())
        && EXCLUDED_FILES.contains(&file_name)
    {
        return true;
    }

    matches!(
        (output_canonical, path.canonicalize().ok()),
        (Some(out), Some(canonical)) if canonical == out
    )
}

fn report_progress(file_count: usize, dropped: usize, token_estimate: usize) {
    eprintln!("----------------------------------------");
    eprintln!("Processed {} files.", file_count);
    if dropped > 0 {
        eprintln!("Omitted {} files to fit the token budget.", dropped);
    }
    eprintln!("Estimated Tokens: ~{}", token_estimate);
    eprintln!("----------------------------------------");
}

fn write_output(
    args: &Args,
    final_output: &str,
    file_count: usize,
    token_estimate: usize,
) -> Result<()> {
    if args.claude {
        let output_path = args
            .output
            .clone()
            .unwrap_or_else(|| PathBuf::from("CLAUDE.md"));
        let wrapped = render::wrap_for_claude_md(final_output, file_count, token_estimate);

        let final_content = if output_path.exists() {
            let existing = std::fs::read_to_string(&output_path)?;
            render::update_or_append_repomap(&existing, &wrapped)
        } else {
            wrapped
        };

        std::fs::write(&output_path, &final_content)?;
        eprintln!("Map successfully written to: {}", output_path.display());
    } else if let Some(output_path) = &args.output {
        std::fs::write(output_path, final_output)?;
        eprintln!("Map successfully written to: {}", output_path.display());
    } else {
        println!("{}", final_output);
    }

    Ok(())
}

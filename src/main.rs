mod formatter;
mod languages;
mod parser;
mod walk;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use formatter::RepoStats;
use languages::Language;

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

fn main() -> Result<()> {
    let args = Args::parse();
    let mut stats = RepoStats::new();

    for result in walk::create_walker(&args.root, args.depth, &args.exclude) {
        let entry = result?;
        let path = entry.path();

        if path.is_file() && !walk::is_binary(path) {
            let target_lang = args.language.or_else(|| languages::infer_language(path));
            if let Some(lang) = target_lang
                && let Ok((file_map, sym_count, line_count)) =
                    formatter::process_file_with_stats(path, lang)
                && !file_map.is_empty()
            {
                stats.add_file(path, file_map, sym_count, line_count);
            }
        }
    }

    let final_output = formatter::assemble_final_map(&args.root, &stats, args.summary);

    eprintln!("----------------------------------------");
    eprintln!("Processed {} files.", stats.file_count);
    eprintln!(
        "Estimated Tokens: ~{}",
        stats.estimate_tokens(&final_output)
    );
    eprintln!("----------------------------------------");

    if let Some(output_path) = args.output {
        std::fs::write(&output_path, &final_output)?;
        println!("Map successfully written to: {}", output_path.display());
    } else {
        println!("{}", final_output);
    }

    Ok(())
}

use std::error::Error;
use std::path::PathBuf;

use clap::Parser;
use loopflow::engine::{
    default_gather_sources, format_prompt, gather_context, trim_context_with_breakdown,
    GatherContextOpts, PromptFormatMode, Surface, DEFAULT_CONTEXT_BUDGET,
};

#[derive(Parser, Debug)]
#[command(name = "lf-prompt")]
#[command(about = "Emit a formatted prompt for parity tests")]
struct Args {
    /// Repository root
    #[arg(long)]
    repo: PathBuf,

    /// Step name
    #[arg(long)]
    step: Option<String>,

    /// Prompt surface (headless/cli/concerto_mac/concerto_iphone)
    #[arg(long)]
    surface: Option<Surface>,

    /// Directions to apply (repeatable)
    #[arg(long = "direction")]
    directions: Vec<String>,

    /// Include lfdocs (scratch/, wave/, root .md)
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    lfdocs: bool,

    /// Include diff files
    #[arg(long = "diff-files", default_value = "true", action = clap::ArgAction::Set)]
    diff_files: bool,

    /// Include unified diff
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    diff: bool,

    /// Include clipboard content
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    clipboard: bool,

    /// Area scope
    #[arg(long)]
    area: Option<String>,

    /// Wave name
    #[arg(long)]
    wave: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let opts = GatherContextOpts {
        repo_root: args.repo,
        step: args.step,
        message: None,
        surface: args.surface.unwrap_or_default(),
        directions: args.directions,
        files: Vec::new(),
        sources: default_gather_sources(args.lfdocs, args.diff_files || args.diff, args.clipboard),
        area: args.area,
        wave: args.wave,
        related_repos: Vec::new(),
    };

    let gathered = gather_context(&opts)?;
    let budgeted = trim_context_with_breakdown(gathered, DEFAULT_CONTEXT_BUDGET);
    let prompt = format_prompt(PromptFormatMode::Full, &budgeted);
    println!("{prompt}");

    Ok(())
}

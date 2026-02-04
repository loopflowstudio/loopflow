use std::error::Error;
use std::path::PathBuf;

use clap::Parser;
use loopflow_engine::{format_prompt, gather_context, GatherContextOpts};

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

    /// Run mode (auto/interactive)
    #[arg(long = "run-mode")]
    run_mode: Option<String>,

    /// Directions to apply (repeatable)
    #[arg(long = "direction")]
    directions: Vec<String>,

    /// Include lfdocs (scratch/, roadmap/, root .md)
    #[arg(long, default_value_t = true, value_parser = clap::builder::BoolishValueParser::new())]
    lfdocs: bool,

    /// Include diff files
    #[arg(
        long = "diff-files",
        default_value_t = true,
        value_parser = clap::builder::BoolishValueParser::new()
    )]
    diff_files: bool,

    /// Include unified diff
    #[arg(long, default_value_t = false, value_parser = clap::builder::BoolishValueParser::new())]
    diff: bool,

    /// Include clipboard content
    #[arg(long, default_value_t = false, value_parser = clap::builder::BoolishValueParser::new())]
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
        inline: None,
        step_args: Vec::new(),
        run_mode: args.run_mode,
        directions: args.directions,
        files: Vec::new(),
        lfdocs: args.lfdocs,
        diff_files: args.diff_files,
        diff: args.diff,
        clipboard: args.clipboard,
        area: args.area,
        wave: args.wave,
    };

    let components = gather_context(&opts)?;
    let prompt = format_prompt(&components);
    println!("{prompt}");

    Ok(())
}

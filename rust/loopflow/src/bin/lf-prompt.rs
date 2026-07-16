use std::error::Error;
use std::path::PathBuf;

use clap::Parser;
use loopflow::engine::{
    drop_native_instruction_docs, format_prompt, gather_context, GatherContextOpts,
    PromptFormatMode, Surface,
};

#[derive(Parser, Debug)]
#[command(name = "lf-prompt")]
#[command(about = "Emit a formatted prompt for parity tests")]
struct Args {
    /// Repository root
    #[arg(long)]
    repo: PathBuf,

    /// Skill name
    #[arg(long)]
    skill: Option<String>,

    /// Prompt surface (headless/cli/mac/iphone)
    #[arg(long)]
    surface: Option<Surface>,

    /// Exclude loopflow operating and Wave planning guidance
    #[arg(long = "no-loopflow")]
    no_loopflow: bool,

    /// Directions to apply (repeatable)
    #[arg(long = "direction")]
    directions: Vec<String>,

    /// Docs paths, globs, or directories to include
    #[arg(long = "docs", value_delimiter = ',')]
    docs: Vec<String>,

    /// Include diff files
    #[arg(long = "diff-files", default_value = "false", action = clap::ArgAction::Set)]
    diff_files: bool,

    /// Include unified diff
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    diff: bool,

    /// Include clipboard content
    #[arg(long, default_value = "false", action = clap::ArgAction::Set)]
    clipboard: bool,

    /// Wave name
    #[arg(long)]
    wave: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let opts = GatherContextOpts {
        repo_root: args.repo.clone(),
        skill: args.skill,
        message: None,
        operate: !args.no_loopflow,
        surface: args.surface.unwrap_or_default(),
        directions: args.directions,
        docs: args.docs,
        files: Vec::new(),
        include_diff: args.diff,
        include_diff_files: args.diff_files,
        include_clipboard: args.clipboard,
        wave: args.wave,
        related_repos: Vec::new(),
    };

    let mut gathered = gather_context(&opts)?;
    let _ = drop_native_instruction_docs(gathered.components_mut(), &args.repo);
    let prompt = format_prompt(PromptFormatMode::Full, gathered.components());
    println!("{prompt}");

    Ok(())
}

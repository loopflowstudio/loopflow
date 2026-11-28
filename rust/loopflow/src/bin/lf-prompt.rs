use std::error::Error;
use std::path::PathBuf;

use clap::Parser;
use loopflow_execution::harness::Mode;
use loopflow_execution::prompt::drop_duplicate_docs;
use loopflow_execution::prompt::format_prompt;
use loopflow_execution::prompt::gather_context;
use loopflow_execution::prompt::ClipboardContext;
use loopflow_execution::prompt::GatherContextOpts;

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

    /// Whether someone is in the conversation the prompt opens
    #[arg(long, value_enum, default_value = "headless")]
    mode: Mode,

    /// Exclude loopflow operating guidance
    #[arg(long = "no-loopflow")]
    no_loopflow: bool,

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
    loopflow::build_info::initialize();
    let args = Args::parse();
    let wave = args
        .wave
        .or_else(loopflow_execution::work::wave::context::resolve_ambient_wave_name);

    let opts = GatherContextOpts {
        repo_root: args.repo.clone(),
        skill: args.skill,
        message: None,
        operate: !args.no_loopflow,
        mode: args.mode,
        docs: args.docs,
        files: Vec::new(),
        include_diff: args.diff,
        include_diff_files: args.diff_files,
        clipboard: if args.clipboard {
            ClipboardContext::Required
        } else {
            ClipboardContext::Off
        },
        wave,
        related_repos: Vec::new(),
    };

    let mut components = gather_context(&opts)?;
    let _ = drop_duplicate_docs(&mut components, &args.repo);
    let prompt = format_prompt(&components);
    println!("{prompt}");

    Ok(())
}

#[cfg(test)]
#[ctor::ctor(unsafe)]
fn initialize_test_build() {
    loopflow_execution::test_support::isolate_from_the_developers_machine();
    loopflow::build_info::initialize();
}

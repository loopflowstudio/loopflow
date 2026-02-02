use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod discovery;
mod output;

#[derive(Parser, Debug)]
#[command(name = "lf")]
#[command(about = "Run steps and flows with coding agents")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// List available steps and flows
    #[arg(short, long)]
    list: bool,

    /// Direction(s) to apply (repeatable or comma-separated)
    #[arg(
        short = 'd',
        long = "direction",
        value_delimiter = ',',
        short_alias = 'D'
    )]
    direction: Vec<String>,

    /// Area scope (paths to include in context)
    #[arg(short = 'a', long = "area", short_alias = 'A')]
    area: Vec<PathBuf>,

    /// Include clipboard content in prompt
    #[arg(short = 'c', long = "clipboard", short_alias = 'C')]
    clipboard: bool,

    /// Model to use (backend or backend:variant)
    #[arg(short = 'm', long = "model", short_alias = 'M')]
    model: Option<String>,

    /// Skip permission prompts
    #[arg(long)]
    yolo: bool,

    /// Run interactively
    #[arg(short = 'i', long = "interactive", short_alias = 'I')]
    interactive: bool,

    /// Run in batch/headless mode
    #[arg(short = 'b', long = "batch", short_alias = 'B')]
    batch: bool,

    /// Copy prompt to clipboard and open web client
    #[arg(long)]
    web: bool,

    /// Enable Chrome integration (Claude)
    #[arg(long)]
    chrome: bool,

    /// Disable Chrome integration (Claude)
    #[arg(long = "no-chrome", overrides_with = "chrome")]
    no_chrome: bool,

    /// Wave name for roadmap scoping
    #[arg(short = 'w', long = "wave", short_alias = 'W')]
    wave: Option<String>,
}

impl Cli {
    /// Get chrome setting: Some(true) if --chrome, Some(false) if --no-chrome, None if neither.
    pub fn chrome_setting(&self) -> Option<bool> {
        if self.chrome {
            Some(true)
        } else if self.no_chrome {
            Some(false)
        } else {
            None
        }
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run a step (explicit)
    Run {
        step: String,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Run a named flow
    Flow {
        name: String,
        #[arg(short = 'a', long = "area", short_alias = 'A')]
        area: Vec<PathBuf>,
        #[arg(short = 'm', long = "model", short_alias = 'M')]
        model: Option<String>,
        #[arg(long)]
        pr: bool,
    },
    /// Run an inline prompt
    #[command(name = ":")]
    Inline {
        #[arg(trailing_var_arg = true)]
        prompt: Vec<String>,
    },
    /// Show assembled context
    Context {
        #[arg(long)]
        tokens: bool,
        #[arg(long)]
        trim: Option<usize>,
    },
    /// Show configuration
    Config {
        #[arg(long)]
        global: bool,
        #[arg(long)]
        repo: bool,
    },
    /// Git operations
    Ops {
        #[command(subcommand)]
        op: OpsCommand,
    },
    /// List available flows
    Flows,
    /// List available steps
    Steps,
    /// List available directions
    Directions,
    /// External: step name (when no subcommand matches)
    #[command(external_subcommand)]
    Step(Vec<String>),
}

#[derive(Subcommand, Debug)]
pub enum OpsCommand {
    Rebase {
        onto: Option<String>,
    },
    Push {
        #[arg(long)]
        force: bool,
    },
    Land {
        #[arg(long)]
        strategy: Option<String>,
    },
    Pr {
        title: Option<String>,
        #[arg(long)]
        draft: bool,
    },
    Sync,
    Next,
    Commit {
        #[arg(short = 'm', long = "message", short_alias = 'M')]
        message: Option<String>,
    },
    Abandon {
        #[arg(long)]
        force: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.list {
        return commands::list::show_all(&cli);
    }

    match &cli.command {
        Some(Commands::Run { step, args }) => commands::step::run(step, args, &cli),
        Some(Commands::Flow {
            name,
            area,
            model,
            pr,
        }) => commands::flow::run(name, area, model.as_deref(), *pr, &cli),
        Some(Commands::Inline { prompt }) => commands::inline::run(prompt, &cli),
        Some(Commands::Context { tokens, trim }) => commands::context::show(*tokens, *trim, &cli),
        Some(Commands::Config { global, repo }) => commands::config::show(*global, *repo),
        Some(Commands::Ops { op }) => commands::ops::run(op),
        Some(Commands::Flows) => commands::list::flows(&cli),
        Some(Commands::Steps) => commands::list::steps(&cli),
        Some(Commands::Directions) => commands::list::directions(&cli),
        Some(Commands::Step(args)) => {
            let (step, step_args) = commands::step::split_step_args(args)?;
            commands::step::run(&step, &step_args, &cli)
        }
        None => commands::step::run_interactive(&cli),
    }
}

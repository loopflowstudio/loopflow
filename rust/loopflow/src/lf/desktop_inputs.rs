//! Desktop command parsing, converted into the shared operation's inputs.
use clap::Args;
use std::path::PathBuf;

/// Private Desktop launch inputs, also carried verbatim across Machine routing.
#[derive(Args, Debug)]
pub struct ShellArgs {
    pub issue: String,
    #[arg(long)]
    pub checkout: std::path::PathBuf,
    #[arg(long)]
    pub id: String,
    /// Create once; retrying the identity never replays startup.
    #[arg(long, conflicts_with = "close")]
    pub create: bool,
    #[arg(long)]
    pub close: bool,
    /// Desktop startup script and relative UTF-8 resources, only for creation.
    #[arg(long, requires = "create")]
    pub bootstrap: Option<String>,
    #[arg(last = true)]
    pub command: Vec<String>,
}

impl ShellArgs {
    pub fn to_request(&self) -> loopflow_execution::desktop::shells::ShellRequest {
        loopflow_execution::desktop::shells::ShellRequest {
            issue: self.issue.clone(),
            checkout: self.checkout.clone(),
            id: self.id.clone(),
            create: self.create,
            close: self.close,
            bootstrap: self.bootstrap.clone(),
            command: self.command.clone(),
        }
    }
}

/// Desktop portfolio command arguments.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum PortfolioArgs {
    /// Discover immediate main checkouts in a folder (defaults to the saved folder)
    Folder { path: Option<PathBuf> },
    /// Use only explicitly selected repositories
    Manual,
    /// Add repositories to the saved manual selection
    Add {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// Remove manual membership, preserving files and open workspaces
    Remove { path: PathBuf },
    /// Import previous Desktop membership only when no selection has been saved
    Import {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
}

impl PortfolioArgs {
    pub fn to_action(&self) -> loopflow_execution::portfolio::PortfolioCommand {
        use loopflow_execution::portfolio::PortfolioCommand;
        match self {
            Self::Folder { path } => PortfolioCommand::Folder { path: path.clone() },
            Self::Manual => PortfolioCommand::Manual,
            Self::Add { paths } => PortfolioCommand::Add {
                paths: paths.clone(),
            },
            Self::Remove { path } => PortfolioCommand::Remove { path: path.clone() },
            Self::Import { paths } => PortfolioCommand::Import {
                paths: paths.clone(),
            },
        }
    }
}

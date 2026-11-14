use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum ProcessCommand {
    /// Print one parseable snapshot of live Loopflow call trees
    List {
        /// Emit the versioned activity snapshot as JSON
        #[arg(long)]
        json: bool,
        /// Print a JSON line now and each time the call trees change, until stdin closes
        #[arg(long, requires = "json")]
        subscribe: bool,
        #[command(flatten)]
        selection: crate::lf::Selection,
    },
    /// Refresh live Loopflow call trees on a terminal; print once when redirected
    Top,
    /// Inspect one Graph that ran: its steps, from their Processes
    Show {
        /// Graph process ID
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Reap orphan providers, remove dead receipts, forget finished Process rows and release unrecorded AgentProcess ancestry
    Prune {
        /// Show exact targets without changing processes, receipts or the store
        #[arg(long)]
        dry_run: bool,
        /// Emit the versioned prune report as JSON
        #[arg(long)]
        json: bool,
    },
}

impl ProcessCommand {
    /// Whether this command changes state, and so records a Process.
    pub fn changes_work(&self) -> bool {
        match self {
            Self::List { .. } | Self::Top | Self::Show { .. } => false,
            Self::Prune { dry_run, .. } => !dry_run,
        }
    }
}

pub fn run(command: &ProcessCommand) -> anyhow::Result<()> {
    match command {
        ProcessCommand::List {
            subscribe: true, ..
        } => super::top::subscribe_ps(super::subscribe::Transport::Stdio).map(drop),
        ProcessCommand::List { json, .. } => super::top::run_ps(*json),
        ProcessCommand::Top => super::top::run_top(),
        ProcessCommand::Show { id, json } => super::graph_inventory::inspect(id, *json),
        ProcessCommand::Prune { dry_run, json } => super::top::run_prune(*json, *dry_run),
    }
}

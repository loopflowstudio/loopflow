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
    /// Move idle Sessions still running a superseded lf onto the selected install
    #[command(
        after_help = "A Session moves when it has no live turn, running Graph or headless Run, and its provider history is saved. Moving an owner discards unsent input, just as quitting its provider does. Viewers don't hold a Session; one may need to reopen it.\n\nA move ends the owner and its agent, then starts `session resume` fresh on the selected lf.\n\nA running Graph isn't stopped: its next step launches on the selected lf, and its runner exits with the Graph. Every other process on a superseded lf, including a runner on Desktop's bundled lf or on an lf that can't open the store, is listed with why it stays.\n\nExit status: 1 when an owner couldn't be stopped or a Session failed to resume; 0 otherwise."
    )]
    Rollover {
        /// List what would move and what stays without stopping anything
        #[arg(long)]
        dry_run: bool,
        /// Emit the rollover report as JSON
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
            Self::Rollover { dry_run, .. } | Self::Prune { dry_run, .. } => !dry_run,
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
        ProcessCommand::Rollover { dry_run, json } => super::rollover::run(*dry_run, *json),
        ProcessCommand::Prune { dry_run, json } => super::top::run_prune(*json, *dry_run),
    }
}

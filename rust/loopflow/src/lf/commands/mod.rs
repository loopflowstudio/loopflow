pub mod account;
pub mod activity;
pub mod bind_attribution;
pub mod ci;
pub mod config;
pub mod context;
pub mod context_cost;
pub mod discord;
pub mod doctor;
pub mod flow;
pub mod flow_inventory;
pub mod install;
pub mod list;
pub mod machine;
pub mod monitor;
pub mod open;
pub mod ops;
pub mod placement;
pub mod profile;
pub mod replay;
pub mod run;
pub mod session;
pub mod session_history;
mod session_watch;
pub mod ssh;
pub mod tokens;
pub mod top;
pub mod usage;
pub mod util;
pub mod waves;
pub(crate) mod work_catalog;
pub mod work_watch;

/// One drill over the Wave → Project → Task Work hierarchy.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WorkFilter<'a> {
    pub wave: Option<&'a str>,
    pub project: Option<&'a str>,
    pub task: Option<&'a str>,
}

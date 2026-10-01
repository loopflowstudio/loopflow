pub mod account;
pub mod activity;
pub mod ask;
pub mod ci;
pub mod desktop;
pub mod discord;
pub mod doctor;
pub mod exec;
pub mod flow;
pub mod flow_inventory;
pub mod home;
pub mod install;
pub mod list;
pub mod monitor;
pub mod ops;
pub mod placement;
pub mod profile;
pub mod replay;
pub mod run;
pub mod runs;
mod runs_watch;
pub mod screenshot;
pub mod session;
pub mod ssh;
pub mod tokens;
pub mod top;
pub mod usage;
pub mod util;
pub mod waves;
pub(crate) mod work_catalog;

/// One drill over the Wave → Project → Task Work hierarchy.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WorkFilter<'a> {
    pub wave: Option<&'a str>,
    pub project: Option<&'a str>,
    pub task: Option<&'a str>,
}

pub mod account;
pub mod activity;
pub mod bind_attribution;
pub mod ci;
pub mod config;
pub mod context;
pub mod context_cost;
pub mod desktop;
pub mod discord;
pub mod doctor;
pub mod graph;
pub mod graph_inventory;
pub mod history;
pub mod history_transcript;
pub mod install;
pub mod list;
pub mod machine;
mod machine_credentials;
pub mod monitor;
pub mod open;
pub mod ops;
pub mod people;
pub mod planning;
pub mod profile;
pub mod rental;
pub mod replay;
pub mod rollover;
pub mod run;
pub mod session;
pub mod session_history;
pub mod session_move;
pub mod setting;
pub mod ship;
pub mod ssh;
pub mod task_create;
pub mod task_row;
pub mod tokens;
pub mod top;
pub mod usage;
pub mod util;
pub mod watch;
pub mod wave_rename;
pub mod waves;
pub(crate) mod work_catalog;
pub mod work_route;

/// One drill over the Wave → Project → Task Work hierarchy.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WorkFilter<'a> {
    pub wave: Option<&'a str>,
    pub project: Option<&'a str>,
    pub task: Option<&'a str>,
}

pub mod agent;
mod agent_input_timing;
pub mod asana;
pub mod build_info;
pub mod builtins;
pub mod chat;
pub mod child;
pub mod command;
pub mod config;
pub mod context_block;
pub mod context_usage;
pub mod definition;
pub mod diagnostic_log;
pub mod durable;
pub mod error;
pub mod git;
pub mod graph;
pub mod harness;
pub mod id;
pub mod installation;
pub mod journal;
pub mod lf;
pub mod machine_records;
pub(crate) mod machine_route;
pub mod naming;
pub(crate) mod os_process;
pub mod platform;
pub mod portfolio;
pub mod process;
pub mod prompt;
pub mod skills;
pub(crate) mod startup;
pub mod task_graph;
pub mod task_work;
pub(crate) mod terminal_output;
pub(crate) mod terminal_title;
// Build-time parsing lives here so its golden tests compile against the exact parser.
pub mod linear;
#[allow(dead_code)]
pub mod notion;
pub mod ops;
pub mod performance;
pub mod planning_exchange;
pub mod planning_git;
pub mod pr_landing;
pub mod profile;
pub mod program_status;
pub(crate) mod promotion_lock;
pub mod provider_account;
pub mod provider_auth;
pub mod rental;
pub mod repo;
pub mod repository;
pub mod security;
pub mod session;
pub(crate) mod session_record;
pub mod steer;
pub mod store;
pub mod subscription;
pub mod trace;
pub mod transcript;
pub mod work;
pub mod world;

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
pub(crate) mod test_support;
#[cfg(test)]
pub(crate) use test_support::ambient as test_ambient;

#[cfg(test)]
extern crate self as loopflow;
#[cfg(test)]
#[path = "../tests/support/linear_input.rs"]
pub(crate) mod test_linear_input;

#[cfg(test)]
#[path = "../tests/linear/graph_tests.rs"]
mod graph_tests;
#[cfg(test)]
#[path = "../tests/linear/local_planning.rs"]
mod local_planning;
#[cfg(test)]
#[path = "../tests/linear/planning_lookup_tests.rs"]
mod planning_lookup_tests;
#[cfg(test)]
#[path = "../tests/linear/pr_tests.rs"]
mod pr_tests;
#[cfg(test)]
#[path = "../tests/linear/repository_team_matrix.rs"]
mod repository_team_matrix;
#[cfg(test)]
#[path = "../tests/linear/status_tests.rs"]
mod status_tests;
#[cfg(test)]
#[path = "../tests/linear/task_adoption_tests.rs"]
mod task_adoption_tests;
#[cfg(test)]
#[path = "../tests/linear/task_graph_launch_tests.rs"]
mod task_graph_launch_tests;
#[cfg(test)]
#[path = "../tests/linear/watch.rs"]
mod watch;
#[cfg(test)]
#[path = "../tests/linear/wave_repository_ownership.rs"]
mod wave_repository_ownership;
#[cfg(test)]
#[path = "../tests/linear/wave_resolution_matrix.rs"]
mod wave_resolution_matrix;
#[cfg(test)]
#[path = "../tests/linear/wave_resolution_tests.rs"]
mod wave_resolution_tests;

#[cfg(unix)]
pub mod transport;

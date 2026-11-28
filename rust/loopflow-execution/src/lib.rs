pub mod agent;
pub mod agent_input_timing;
pub mod asana;
pub mod build_identity;
pub mod chat;
pub mod child;
pub mod command;
pub mod config;
pub mod context_block;
pub mod context_usage;
pub mod definition;
pub mod desktop;
pub mod desktop_exits;
pub mod diagnostic_log;
pub mod durable;
pub mod engine;
pub mod error;
pub mod git;
pub mod graph;
pub mod harness;
pub mod installation;
pub mod journal;
pub mod linear;
pub mod machine;
pub mod machine_records;
pub mod machine_route;
pub mod naming;
pub mod observe;
pub mod ops;
pub mod os_process;
pub mod output;
pub mod performance;
pub mod platform;
pub mod portfolio;
pub mod pr_landing;
pub mod process;
pub mod profile;
pub mod promotion_lock;
pub mod prompt;
pub mod provider_auth;
pub mod read_stream;
pub mod rental;
pub mod repo;
pub mod repository;
pub mod runtime;
pub mod security;
pub mod session;
pub mod session_record;
pub mod skills;
pub mod startup;
pub mod steer;
pub mod store;
pub mod task_graph;
pub mod task_work;
pub mod terminal_holder;
pub mod terminal_host;
pub mod terminal_output;
pub mod terminal_title;
pub mod trace;
pub mod transcript;
pub mod work;
pub use loopflow_definitions::builtins;
pub use loopflow_model::{id, program_status};
#[cfg(unix)]
pub mod transport;

#[cfg(any(test, feature = "test-support"))]
#[path = "../tests/support/mod.rs"]
pub mod test_support;
#[cfg(any(test, feature = "test-support"))]
pub use test_support::ambient as test_ambient;
#[cfg(any(test, feature = "test-support"))]
extern crate self as loopflow;
#[cfg(any(test, feature = "test-support"))]
#[path = "../tests/support/linear_input.rs"]
pub mod test_linear_input;

#[cfg(test)]
#[ctor::ctor(unsafe)]
fn initialize_fixture_build() {
    test_support::isolate_from_the_developers_machine();
    build_identity::initialize(build_identity::BuildIdentity {
        provenance: build_identity::BuildProvenance::Development,
        source_root: Some(std::path::Path::new("/fixture/loopflow-execution")),
        source_revision: "fixture",
        version: env!("CARGO_PKG_VERSION"),
        label: None,
    });
}

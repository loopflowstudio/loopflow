pub mod build_info;
#[cfg(test)]
mod id_tests;
pub mod lf;
pub mod server;

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
#[path = "../tests/work/graph_tests.rs"]
mod graph_tests;
#[cfg(test)]
#[path = "../tests/work/local_planning.rs"]
mod local_planning;
#[cfg(test)]
#[path = "../tests/work/planning_lookup_tests.rs"]
mod planning_lookup_tests;
#[cfg(test)]
#[path = "../tests/work/pr_tests.rs"]
mod pr_tests;
#[cfg(test)]
#[path = "../tests/work/repository_team_matrix.rs"]
mod repository_team_matrix;
#[cfg(test)]
#[path = "../tests/work/status_tests.rs"]
mod status_tests;
#[cfg(test)]
#[path = "../tests/work/subscribe.rs"]
mod subscribe;
#[cfg(test)]
#[path = "../tests/work/task_adoption_tests.rs"]
mod task_adoption_tests;
#[cfg(test)]
#[path = "../tests/work/task_graph_launch_tests.rs"]
mod task_graph_launch_tests;
#[cfg(test)]
#[path = "../tests/work/wave_repository_ownership.rs"]
mod wave_repository_ownership;
#[cfg(test)]
#[path = "../tests/work/wave_resolution_matrix.rs"]
mod wave_resolution_matrix;
#[cfg(test)]
#[path = "../tests/work/wave_resolution_tests.rs"]
mod wave_resolution_tests;

#[cfg(test)]
mod adapter_tests;

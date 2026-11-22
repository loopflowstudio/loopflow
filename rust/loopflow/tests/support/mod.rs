#[allow(unused_imports)] // Each test binary uses a different subset of these fixtures.
pub use loopflow_execution::test_support::*;

#[ctor::ctor(unsafe)]
fn initialize_application_build() {
    loopflow_execution::test_support::isolate_from_the_developers_machine();
    loopflow::build_info::initialize();
    // An integration binary runs the `lf` Cargo built beside it. Building it
    // again from inside the test would link another feature set over that
    // path while this binary's Machine server still spawns from it.
    if let (Some(built), None) = (
        option_env!("CARGO_BIN_EXE_lf"),
        std::env::var_os("LOOPFLOW_TEST_LF"),
    ) {
        std::env::set_var("LOOPFLOW_TEST_LF", built);
    }
}

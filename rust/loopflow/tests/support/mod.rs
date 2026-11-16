#[allow(unused_imports)] // Each test binary uses a different subset of these fixtures.
pub use loopflow_execution::test_support::*;

#[ctor::ctor(unsafe)]
fn initialize_application_build() {
    loopflow_execution::test_support::isolate_from_the_developers_machine();
    loopflow::build_info::initialize();
}

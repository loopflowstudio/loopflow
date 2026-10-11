# LOO-450 CI repair

Jack Heart requested CI repair after approving landing and quieter headless output.
Failed published head: `cf298370ff98d4d71ff1b1ca024112bc74745ae3`;
GitHub run `38114156447`, job `114395615328`.

Both failures were obsolete fake-Claude protocols: concurrent research artifacts
and Linux public Flow planning reconnect emitted a result without consuming stdin,
racing the harness seed write with process exit. Stand-ins now consume and echo the
request before preserving their existing artifact/reconnect effects and reporting
completion. Production code is unchanged. TESTING.md includes both boundary suites.

Checks: network-isolated `cargo test --offline -p loopflow --lib lf::commands::run::tests::two_task_research_runs_leave_distinct_uncommitted_artifacts_for_the_next_prompt -- --exact`, disconnected Linux container `cargo test --offline -p loopflow --test planning_reconnect_tests public_flow_reconnects_planning_without_another_turn -- --exact --nocapture`, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings` passed; full materialized matrix remains CI-owned.

Review: both fixtures keep their outcome assertions and wait for real input before readiness/completion; no production abstraction or retry added.

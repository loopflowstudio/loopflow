# LOO-415 CI repair

Recorded failed head: `9a4956988742e755d6f29c5c8479fd6913d9923f`, run `39031357833`, PR #1581. Synced main through `d476a8cfb6`; regenerated third-party inventory to resolve the only conflict.

Repair: release Command-owned PTY slaves after spawn; use a raw winsize pointer for the differing libc signatures; follow recorded causal ancestry through retained transport owners in the disposable Task proof. Existing Flow and transport tests cover final output and exit. TESTING.md adds Linux verification for retained terminals.

Remaining: reproduce Linux failures, verify fixes and both platform lints, inspect complete repair, commit through lf and arm the existing PR. Jack Heart's timeout-policy work remains in LOO-503. No CI rerun, gate rerun, merge wait or Task completion belongs here.

Verification: `cargo fmt --all -- --check` PASS; `cargo clippy --all-targets -- -D warnings` PASS on macOS and disposable Linux; network-isolated `cargo test -p loopflow --test transport_tests` PASS 10/10 on each platform; `cargo test -p loopflow --lib flow_tests::interactive_flow_keeps_input_and_advances_only_after_each_provider_exits -- --exact` PASS on each platform; disposable Linux `cargo test -p loopflow --test session_lifecycle_tests declared_agent_can_start_another_tasks_flow -- --exact --include-ignored --nocapture` PASS; `uv run pytest python/tests/test_third_party.py` PASS 13/13. All three recorded failures reproduced on synced code before the repair. Logs: `/tmp/loo415-ci/`.

Review: the repair closes parent-owned descriptors at the spawn boundary, preserves child ownership and final output draining, and checks causal provenance without assuming transport depth. No new abstraction or compatibility path. The complete repair is three files plus the required upstream merge. No remaining review findings.

# LOO-880 gate

Pursue runs kickoff → write-docs → implement → concept-review → refresh
(sync, realign) → loop-or-next → compress → qa → loop-or-next → pr-publish.
Both Iterate paths return to implement. Stopping facilities remain outside this change.

Verification plan:
- `pursue.yaml`, graph tests: `test(graph::)` checks expanded order, both return targets and graph compilation; `test(graph_discovery_tests::)` checks public graph rendering; `test(skills::catalog::)` checks builtin discovery.
- Embedded `launch-plan.md`: `binary(golden_prompt)` checks rendered prompt fixtures; `test(documented_commands::)` checks maintained examples, including the corrected orchestration example.
- `docs/orchestration.md`: changed-aware architecture and website checks.
- Changed-aware runner with passing-result reuse also checks inventory, formatting and clippy; `cargo package --locked --allow-dirty -p loopflow` verifies embedded resources in the extracted archive.

Check results:
- `uv run python scripts/test.py --reuse-passing --rust-filter 'test(graph::) | test(graph_discovery_tests::) | test(skills::catalog::) | test(documented_commands::) | binary(golden_prompt)'`: stopped before suites; uv-cache inventory exceeded its 15-second deadline (35.4 GiB free, above 32 GiB reserve); following scripts/testing.md's direct-command fallback.
- `uv run python scripts/check_architecture.py`: passed, zero unexplained owners/mirrors/shims/stale vocabulary.
- `uv run --no-sync python scripts/check_rust_test_inventory.py`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo build -p loopflow --bin lf --jobs 4`: passed (1m 44s).

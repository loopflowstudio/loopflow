# LOO-450 foreground fixture correction

Jack Heart requested CI repair. Failed SHA `08f69aeba0`, run `38121950452`,
job `114418337338`: 2,344 tests passed; the foreground-close fixture alone failed.
The prior launch-mode API update incorrectly passed `false` for that foreground
fixture. Restore `true`, keeping all refusal, liveness and history assertions.
Production launch sites already pass their actual mode. No production change.

Checks: exact failure reproduced locally before repair; network-isolated `cargo test --offline -p loopflow --lib session_record::runtime::tests -- --test-threads=1`, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings` pass afterward. Full matrix remains CI-owned.

Review: only the fixture launch flag changes; the native-process refusal assertions remain unchanged. TESTING.md now names the whole runtime module for launch-mode changes.

# CI repair

Failed head: 444bcdd0a1e73fc088cd019d4df7d81800740911, PR #1882.
- CI run 42974499587, integration shard 1: duplicate opening returned `Session is attached in another viewer` after successful attachment; its store update is asynchronous.
- CI run 42974499587, integration shard 2: Linux Desktop refusal passed, but the assertion expected the removed `Run: lf open <url>` fallback.
- macOS `cargo test -p loopflow --test transport_tests conversation_open_here_confirms_attachment_and_does_not_take_existing_input`: passed normally, reproduced CI failure after forcing the stored viewer flag stale.
- macOS `cargo test -p loopflow --test transport_tests conversation_open_`: 3 passed after repair.
- macOS `cargo test -p loopflow --test integration work_open_tests::`: 34 passed, 2 installed-host proofs ignored.
- Linux `cargo test -p loopflow --test integration work_open_tests::linux_automatic_route_never_selects_desktop -- --exact`: reproduced old assertion, passed after rebuilding corrected test (Docker copy retains source mtime).
- Linux `cargo test -p loopflow --test integration work_open_tests::`: other 32 passed, 2 installed-host proofs ignored; corrected Linux-only case rerun separately.
- Linux `cargo test -p loopflow --test transport_tests conversation_open_`: 3 passed.
- Linux `cargo test -p loopflow --test integration documented_commands::`: 4 passed.
- `cargo fmt` and Linux `cargo fmt --check`: passed.
- Linux `cargo clippy --all-targets -- -D warnings`: passed. Disposable Linux container removed.

# Terminal preview readiness repair

Queue run 43796858212 failed cancellation at `viewer_tests.rs:396` after the PR
matrix passed: the pending file could arrive before preview bytes were consumed.
The fixture now emits an in-stream marker and cancellation waits for both
observations. One-byte PTY reads exercise fragmentation.

Focused nextest cancellation check: 1 passed; `cargo fmt --check` and
`cargo clippy --all-targets --jobs 1 -- -D warnings`: passed.

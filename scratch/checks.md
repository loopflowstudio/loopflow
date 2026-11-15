cargo test -p loopflow --lib interrupted_paste -- --nocapture — baseline failed waiting for the missing paste-end marker.
cargo test -p loopflow --lib paste -- --nocapture — 9 passed on the final working tree, including the split-paste and exit-receipt fixtures.
cargo clippy --all-targets -- -D warnings — stopped at the 15-minute local budget; unfinished, CI must run it.
uv run python scripts/check_architecture.py — passed locally.
uv run python scripts/check_rust_test_inventory.py — passed locally.
cargo test -p loopflow --lib server::holder::tests:: -- --skip a_lost_writer_finishes_its_paste_without_submitting_it — 11 passed; existing PTY input, output, exit and lifetime behavior.
cargo test -p loopflow --lib transport::registry::tests:: -- --skip an_interrupted_paste_ends_before_the_next_view_types — 11 passed; existing reconnect, replay and settlement behavior.
cargo test -p loopflow --lib transport::tests:: — 8 passed; existing relay, ownership, heartbeat and bounded events.
cargo fmt --all -- --check — passed.
Full transport, documentation-command and iOS checks — CI after publishing.

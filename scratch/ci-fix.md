# LOO-790 CI repair

Failed head: `b239a635d891137e28681c83b71aa0b9e641473b`.
CI run 42739267180, job 129577292190: 313 passed; graph lookup failed with
`Not a directory (os error 20)` and the transport viewer wait failed with
`InvalidColumnType(0, "viewed", Null)`. The latest main CI run, 42733784226,
was successful. `lf sync` left HEAD unchanged.

- Focused nextest reproduction of both failing cases: both failed locally with the same errors.
- Verification scope: graph discovery and parser callers, observed cache behavior, skill catalog traversal, and both session-upgrade tests using `wait_viewed`; full clippy and formatting before commit.
- `cargo nextest run -p loopflow --lib --test integration --test transport_tests -E 'test(graph_discovery_tests::) | test(graph::tests::) | test(observe::tests::) | test(skills::catalog::tests::) | test(unanswering_superseded_owner_preserves_uncertain_draft_and_pending_turn) | test(resumed_session_completes_turns_after_build_replacement_and_viewer_return)' --no-fail-fast`: 83 passed; the macOS linker warned about the large debug unwind section.
- `cargo fmt --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `git diff --check`: passed.

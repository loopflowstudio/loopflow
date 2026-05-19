path: done

Ship-ready. The branch delivers two complete, observable milestones —
lfd-owned embedded terminal sessions and native-chat M1 markdown rendering —
each end-to-end and test-backed.

## Validation (headless, all green)

| Gate | Command | Result |
|------|---------|--------|
| Rust fmt | `cargo fmt --check` | pass |
| Rust lint | `cargo clippy --all-targets -- -D warnings` | pass (0) |
| Rust tests | `cargo test --all` | pass (0) |
| Python | `uv run pytest python/tests/` | 139 passed |
| Swift package | `swift test --package-path swift` | 336 passed (incl. MarkdownBlock done-when) |
| E2E smoke | `tests/e2e/test_smoke.sh` | pass |
| API/WebSocket E2E | `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` | 16 passed |
| Docker smoke | `docker version && cargo test -p loopflow docker_` | 11 passed |
| Embedded terminal behavior | `uv run python scripts/verify_embedded_build_driver.py --skip-build` | OK: failed session stayed attachable |

The Concerto UI runner (`xcodebuild test`) cannot bootstrap in a headless
environment with no rendering session — a documented limitation, exercised by
CI's macOS runner. Swift package/unit tests, which cover the new
`MarkdownBlock`/parser/highlighter logic, pass here.

## Polish state

Code was already polished by the prior gate commit (`e1dd8b7f`); no production
code changed since then. Subsequent commits are review/demo routing docs.
The test-only `RunIdEnvGuard` in `ingest.rs` correctly isolates the
headless `LF_RUN_ID` interaction the wave memory flagged. `pr-body.md`,
`pr-title.txt`, and the review doc remain accurate.
`scratch/.pr-copy-ref` should be refreshed uncommitted immediately before
`lf op land`, because any commit containing that file necessarily makes its
stored SHA stale.

No reason to iterate — done-when checks pass, docs are reviewer-ready, scope
is clean (history M2 / composer M3 explicitly deferred).

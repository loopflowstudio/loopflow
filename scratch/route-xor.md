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
| Embedded terminal behavior | `scripts/verify_embedded_build_driver.py` | OK: failed session stayed attachable |

The Concerto UI runner (`xcodebuild test`) cannot bootstrap in a headless
environment with no rendering session — a documented limitation, exercised by
CI's macOS runner. Swift package/unit tests, which cover the new
`MarkdownBlock`/parser/highlighter logic, pass here.

## Polish state

Code was already polished by the prior gate commit (`e1dd8b7f`); no production
code changed since (`git diff 2aac3aef..HEAD` is scratch-only). The test-only
`RunIdEnvGuard` in `ingest.rs` correctly isolates the headless `LF_RUN_ID`
interaction the wave memory flagged. Refreshed the stale
`scratch/.pr-copy-ref` to current HEAD; `pr-body.md`, `pr-title.txt`, and the
review doc remain accurate.

No reason to iterate — done-when checks pass, docs are reviewer-ready, scope
is clean (history M2 / composer M3 explicitly deferred).

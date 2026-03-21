## Try it!

```bash
cargo run --bin lf -- ops pm --help
cargo run --bin lf -- ops pm pull --help
cargo run --bin lf -- ops pm export --help
cargo test -p loopflow ops::pm::tests::pm_export_creates_updates_and_skips_without_recreating_missing_remote_items -- --exact
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
uv run pytest python/tests/
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift
```

What to look for:
- `lf ops pm --help` now advertises the full PM surface, including `pull` and `export`.
- `lf ops pm pull --help` and `lf ops pm export --help` show the wave/`--all` entrypoints used by the new built-in PM steps.
- The focused PM export test proves the local-wins path creates new remote items, updates changed ones, and skips missing remote rows instead of recreating them.
- The broad Rust/Python/E2E/Swift package suites pass locally.

Validation summary:
- `cargo fmt --check` ✅
- `cargo clippy --all-targets --all-features -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅ (`115 passed`)
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (`16 passed`)
- `swift test --package-path swift` ✅
- `xcodebuild test ...` built successfully but did not terminate during this local gate run.

## Intent

Ship the PM branch as a coherent user-facing surface: loopflow can now bootstrap waves from PM, pull or export wave state explicitly, compose PM sync into flows, and carry richer runtime state through the daemon and Concerto’s terminal-native workspace UI. The goal is one path from external PM systems into wave execution and back out again, without splitting the implementation across duplicate sync layers or ad hoc UI/runtime state.

## Assumptions

- PR-oriented executor runs should keep using executor-managed `pm_sync`; the new `import-pm`, `export-pm`, and `pm-sync` surfaces are for explicit/manual composition.
- Live Asana/Linear verification still depends on local credentials plus correctly configured provider metadata (`asana.workspace`, `asana.default_team`, `linear.team`, wave `pm` blocks).
- Concerto workspace behavior is primarily relevant on macOS hosts with Ghostty/tmux available.
- This branch is easiest to review by subsystem: PM sync, flow/runtime plumbing, then Concerto workspace/UI.

## Key decisions

- Kept PM operations directional: `pull` is remote-wins, `export` is local-wins, `sync` remains the three-way merge path.
- Reused shared PM provider seams and frontmatter helpers instead of introducing second implementations inside built-in steps.
- Made export additive-only: no destructive deletes/completion and no fake ordering sync where provider semantics are weak.
- Stored terminal/journal state in the daemon so Concerto’s workspace and multiplexer views render from durable runtime state, not local-only heuristics.
- Polished docs/help so the shipped command and step names line up with the actual behavior (`pull`, not team-level `import`, for `import-pm`).

## Not included

- Destructive PM export behavior for remote items missing locally.
- Full ordering parity across PM providers.
- A replacement for executor lifecycle sync.
- Full Notion parity beyond the groundwork/docs already added in `wave/pm/`.

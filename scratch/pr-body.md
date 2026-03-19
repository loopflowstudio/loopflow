## Try it!

- `cargo test --all`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `lf ops pm init pm` — bootstrap one PM-linked wave from the configured read/write provider
- `lf ops pm pull pm` — rewrite the local `wave/pm/` files from remote PM priority order
- `uv run python scripts/concerto-dev.py run-debug` — launch `lfd` + Concerto, then attach a waiting wave terminal and confirm the terminal workspace auto-presents for that wave

Validation highlights:
- Rust fmt, clippy, full tests, and docker-targeted tests passed locally
- Python unit tests passed (`115 passed`)
- E2E smoke + API/concurrent-client tests passed (`16 passed`)
- Swift package tests passed locally
- `xcodebuild test` still ends with `ConcertoUITests-Runner ... Early unexpected exit` on this machine after the app/unit suites complete; see Assumptions below

## Intent

This branch does two connected pieces of runtime work. First, it gives `lfd` and Concerto a typed terminal-session model so embedded terminal workspaces can be created, attached, auto-focused, and completed without scraping terminal text. Second, it reshapes PM sync around explicit provider roles, adds deterministic `pm init/pull/status` entrypoints, and makes Linear pulls honor remote priority order so roadmap files line up with the PM source of truth.

## Assumptions

- `.lf/config.yaml` (or repo config) defines a valid PM read/write provider, and provider-specific settings such as `linear.team` / `asana.workspace` are already configured.
- Remote PM state is the source of truth for `lf ops pm pull`; local wave markdown is expected to be disposable/regenerable.
- Terminal completion callbacks can reach the local daemon over HTTP from the launched shell environment.
- The local `ConcertoUITests` runner crash is environment-specific or existing test-runner instability, not a terminal-session logic regression; CI should confirm that before merge.

## Key decisions

- Kept PM sync on the shared `lfd::pm` provider seam and removed the separate legacy export path instead of maintaining two orchestration surfaces.
- Modeled terminal sessions as explicit pending/attached/running/completed/cancelled state with typed DTOs and events, which keeps Swift and Rust in sync and makes recovery/replay possible.
- Sorted Linear items by `prioritySortOrder` first and `sortOrder` second, then reordered the local `wave/agent-embedding/` items to match the remote PM priority that users see.
- Hardened branch rename logic so moved worktrees can still be renamed reliably during wave/worktree lifecycle operations.

## Not included

- No daemon-owned PTY transport or live terminal streaming beyond launch/attach/complete bookkeeping.
- No Notion provider implementation.
- No local fix for the `ConcertoUITests` runner bootstrap crash.

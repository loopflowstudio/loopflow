# Review: terminal workspace runtime + PM pull/init sync

## What was implemented

- Added durable terminal-session state to `lfd`, including store migrations, typed HTTP routes, event fanout, and a shell wrapper that reports terminal exit back through a completion token.
- Wired Concerto and LoopflowCore to that runtime with new terminal-session models, `TerminalWorkspaceStore`, terminal auto-present/focus behavior, and the new `TerminalWorkspaceView` / `WaveWorkspaceView` surfaces.
- Reworked PM tooling around explicit provider roles: `lf ops pm init`, `lf ops pm pull`, and `lf ops pm status` now operate through the shared PM provider seam, and Linear pulls now respect `prioritySortOrder` so local wave item order matches remote priority.
- Reordered the `wave/agent-embedding/` roadmap files to match the remote priority order after the Linear sort fix.
- Hardened git branch renames for moved worktrees by falling back to renaming from the checked-out worktree when `git branch -m old new` cannot resolve the branch from the main repo.

## Key choices

- **Provider roles, not parallel sync paths.** The branch keeps one read/write PM source (`rw_provider`) and optional mirror exporters (`export_providers`) instead of reviving the removed standalone export command surface.
- **Remote-wins pull for roadmap refresh.** `lf ops pm pull` rewrites local wave files directly from PM state, which keeps ingest/bootstrap deterministic and avoids a second merge policy.
- **Terminal sessions as explicit state machines.** Attach/start/complete/cancel are modeled in Rust and Swift instead of inferred from terminal text, which lets Concerto route, persist, and recover sessions coherently.
- **Best-effort terminal completion reporting.** The shell wrapper preserves the child process exit code even if the completion callback fails, favoring terminal correctness over strict callback delivery.

## How it fits together

`lfd` now persists `TerminalSession` records, exposes typed `/v0/terminal-sessions/*` routes, and emits session updates through the event hub. LoopflowCore consumes those routes and events through `LocalWaveService`, `RepoState`, and `TerminalWorkspaceStore`, which lets Concerto auto-focus a wave, surface its embedded terminal workspace, and keep UI state in sync with the daemon. On the PM side, `ops/pm.rs` orchestrates provider-role bootstrap/pull/status flows on top of the shared `lfd::pm` provider trait, with Linear ordering flowing all the way through to local wave file order.

## Risks and bottlenecks

- The local macOS UI test command still fails at the `ConcertoUITests-Runner` bootstrap stage even though the package tests and app/unit tests pass; this needs CI verification before landing.
- Terminal completion depends on an in-terminal `curl` callback with a short-lived token. If that callback is blocked, the command still exits correctly, but the daemon can be left with a stale running session until a later reconciliation step.
- `lf ops pm pull` is intentionally remote-wins and rewrites roadmap ordering; anyone with uncommitted local edits in `wave/*` can lose those edits.
- The PM bootstrap path assumes provider configuration is present and actionable. The error messages are better than before, but misconfigured teams/workspaces are still a user-facing setup footgun.

## What's not included

- No daemon-hosted PTY transport or bidirectional terminal streaming; this branch only adds typed launch/attach/complete session state around local terminal execution.
- No Notion provider implementation yet; the branch extends the PM seam and docs, but shipped behavior is still Linear + Asana focused.
- No fix for the local `ConcertoUITests` runner crash in this gate.

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `docker version`
- `cargo test -p loopflow docker_`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -derivedDataPath /tmp/loopflow-ui-test -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` → app/unit suites passed, but `ConcertoUITests-Runner` exited early before bootstrap (`signal kill` / `operation never finished bootstrapping`)

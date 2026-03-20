# jack-heart.lfd.20260319_1333 review

## What was implemented

Added a shared runtime-journal substrate for wave-attributed CLI runs. `lf` now writes per-run metadata and JSONL lifecycle events under `.lf/runtime/runs/<run_id>/`, and `lfd` polls those journals so manual CLI runs show up as first-class `run.*` / `step.*` daemon events. This gate pass also fixed a ship blocker: runtime journal files are now auto-ignored through each worktree's git exclude so strict `lf ops land` and other cleanliness checks do not fail after a journaled CLI run.

## Key choices

- Put the journal schema in shared Rust runtime code so `lf` writes and `lfd` reads the same contract.
- Kept journal emission wave-only by deriving wave identity from sibling worktree naming instead of logging every CLI invocation.
- Mapped journal records onto new daemon-side `run.*` / `step.*` events instead of contorting them into the older wave/agent event vocabulary.
- Auto-added `.lf/runtime/` to git excludes at run start rather than weakening strict git checks; the journal remains durable without polluting worktree status.

## How it fits together

`src/bin/lf.rs` wraps command execution in `RuntimeRun`, which writes `meta.json` plus `events.jsonl` entries from `runtime/mod.rs`. Flow execution emits step lifecycle markers, and `lfd/runtime_journal.rs` scans known wave worktrees, replays unseen journal lines, and publishes mapped websocket events through `EventHub`. The git-exclude helper runs before journal creation so the observability path stays invisible to normal worktree hygiene.

## Risks and bottlenecks

- `lfd` currently polls runtime journals once per second, so visibility is near-real-time but not push-based.
- Journal ingestion trusts per-line JSON parsing; malformed lines are skipped, but a broken writer would still hide run state.
- Concerto macOS UI tests still fail on this host because `ConcertoUITests-Runner` exits during bootstrap before any UI assertions run.

## What's not included

- No alternate journal root override; journals still live at `<worktree>/.lf/runtime/runs/...`.
- No daemon-hosted shell / PTY work for CLI runs.
- No fix for the current macOS UI-test bootstrap crash.

## Validation

- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅
- `swift test --package-path swift` ✅
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ❌ (`ConcertoUITests-Runner` early bootstrap exit, reproduced twice)

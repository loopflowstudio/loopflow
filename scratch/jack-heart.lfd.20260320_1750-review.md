# Review: lfd real CLI executor groundwork

## What was implemented

- `journal::emit()` now honors `LF_RUN_ID` when the environment provides a valid `LfdId`, so daemon-supervised CLI runs can write into the daemon's run directory instead of inventing a second journal id.
- Invalid `LF_RUN_ID` values are ignored with a debug log and fall back to a generated id, which keeps manual CLI runs and bad env injection from breaking journal writes.
- `LfObserver` now reuses the shared `journal::read_events()` helper instead of maintaining a second JSONL reader.
- The unbuilt real-CLI executor design moved from `wave/lfd/01-real-cli-executor.md` into `scratch/lfd-real-cli-executor.md`, and `scratch/questions.md` now records that this branch only lands the journal-correlation slice.

## Key choices

- Validate `LF_RUN_ID` with `LfdId::parse` instead of trusting arbitrary strings; bad daemon state should degrade to generated journal ids, not break runs.
- Reuse the shared journal parser inside `LfObserver` so the CLI and daemon do not drift on event decoding.
- Treat the larger shared-`FlowEngine` refactor as design work for a later branch, not something to half-land behind the already-shipped journal contract.

## How it fits together

`lfd` already stores canonical run ids and already replays CLI journal events. This branch connects those two halves: when the daemon eventually spawns `lf`, it can pass `LF_RUN_ID`, the CLI journal will write under that exact run directory, and the existing observer will fan those events straight back into daemon/websocket event streams without any run-id reconciliation layer.

## Risks and bottlenecks

- The actual daemon-side real-CLI executor work is still outstanding; this branch only lands the journal correlation hook and the design doc for the larger refactor.
- Local macOS UI validation still fails in `xcodebuild test` because `ConcertoUITests-Runner` exits before it finishes bootstrapping. Swift package tests pass, so this looks isolated to the UI-test harness, but CI will need to confirm.
- `LfObserver` still rereads the full event file on each poll; this branch reduces parser duplication, not polling cost.

## What's not included

- No shared `FlowEngine` / `StepExecutor` extraction.
- No daemon process-supervision rewrite to launch `lf <step>` or hosted tmux sessions.
- No daemon-side `LF_RUN_ID`/`LFD_RUN_ID` injection yet; this branch only makes the CLI honor `LF_RUN_ID` once a later branch sets it.
- No fix for the local `ConcertoUITests-Runner` bootstrap crash.

## Validation

- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅
- `swift test --package-path swift` ✅
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ❌ twice, both with `ConcertoUITests-Runner ... crashed with signal kill before establishing connection`

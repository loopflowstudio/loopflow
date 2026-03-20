# jack-heart.lfd.20260319_1333 review

## What was implemented

This branch turns loopflow into a more complete end-to-end system instead of a loose set of features.

- `lf` and `lfd` now share a journal-based runtime contract so manual CLI runs and daemon-launched runs can be observed through the same event model.
- The flow engine grows real branching and governance structure: `xor`, looped build flows, renamed `garden` wave maintenance, and VSM governance flows/steps.
- PM support expands from basic exports to real initialization, pull/status flows, priority buckets, and provider-specific sync behavior.
- Concerto shifts from a single-detail view toward a workspace client with a portfolio dashboard, attention queue, terminal workspace, multiplexer state, and stronger local auth/connection handling.

## Key choices

- **Keep `lf` as the execution engine.** `lfd` supervises and reacts, but normal `lf` commands still own flow semantics and step execution. That avoids a second executor model drifting out of sync.
- **Replace runtime meta files with structured journal events.** The branch deletes the older runtime wrapper objects in favor of a single event schema that `lf` emits and `lfd` replays directly.
- **Model wave orchestration explicitly in flows.** Instead of overloading older `tend`/ship flows, the branch introduces `build`, `garden`, `algedonic`, and VSM governance flows that make routing and looping visible in config.
- **Keep terminal UX local-first.** Concerto gets terminal workspace and multiplexer support now, while daemon-hosted PTYs remain future work rather than blocking the UI shape.

## How it fits together

`lf` expands flows, executes steps, and emits lifecycle events into `.lf/journal/` when a wave worktree provides that runtime store. `lfd` watches and replays those events, layers on scheduling/PM/auth/HTTP state, and exposes live updates to clients. Concerto consumes the same daemon state to render portfolio, attention, run history, and terminal workspace UI without inventing a separate launch protocol.

## Risks and bottlenecks

- **UI test blocker:** the macOS Xcode UI test target still fails to bootstrap `ConcertoUITests-Runner` on a clean run, so the branch is not fully green on the last CI-style check.
- **Breadth:** this branch spans flow semantics, daemon runtime, PM sync, docs, and Swift UI. Reviewers should read it as an integrated milestone, not as an isolated journal-only change.
- **Future journal escalation support:** the event schema supports `*.escalated`, but current CLI error paths still map ordinary failures to `*.errored` until a dedicated escalation signal type exists.
- **Terminal transport remains local-first:** remote or daemon-owned PTYs are intentionally deferred, so terminal behavior still assumes a local Ghostty/tmux workflow.

## What's not included

- No daemon-hosted PTY transport yet.
- No dedicated CLI escalation signal type yet.
- No fix for the existing Concerto UI-test bootstrap failure.
- No backwards-compatibility shims for old wave/flow naming beyond the migrated branch content already in this diff.

## Validation

The branch has no `scratch/jack-heart.lfd.20260319_1333.md` design doc, so I used the repo's documented CI matrix as the gate.

- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `cargo test -p loopflow docker_` ✅
- `uv run pytest python/tests/` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅
- `tests/e2e/test_smoke.sh` ✅
- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate && xcodebuild clean test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ❌ on March 20, 2026 (`ConcertoUITests-Runner` exited before establishing a test connection: `Early unexpected exit, operation never finished bootstrapping`)

A non-clean `xcodebuild test` run also hit a stale DerivedData linker write failure for `ConcertoUITests`, but the clean rerun reproduced the older bootstrap-exit problem above.

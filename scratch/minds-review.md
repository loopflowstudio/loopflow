# Minds design review

## What was implemented

This branch turns a wave into one persistent, inspectable mind rather than a
sequence of disconnected agent runs. `lf loop <wave>` owns a durable thread and
playhead; the playhead loops the wave flow, supports nested FIFO flow queues and
skip, survives restart, and records which disposable body produced each span.

The same execution primitive now covers foreground work and server-owned
detached work. Memory inherits down the wave chain while chat remains local,
`lf project promote` creates a new resident child wave, and `lf chat --steer`
reaches the live Codex body when possible. Loopflow Mac renders the resulting
thread, current route, KRs, open PRs, active sessions, and filed backlog on one
wave screen.

The gate found and fixed two recovery/display defects:

- Force-finalizing a dead resident now closes both its chat turn and active
  playhead body, preserving a retry of the same logical step across respawn.
- `lf status` now carries live PR state/title, including draft state, so the Mac
  Open PRs pane can include completed runs whose PR is still open.

## Key choices

- The journal is the source of truth. Sessions and resident bodies are
  replaceable; replay reconstructs the thread, playhead stack, queue, and return
  point.
- Waves are the only persistent minds. `lf loop <flow> "…"` inhabits work in the
  foreground; `--detach` buys concurrency without inventing another
  conversation surface.
- Enqueued flows belong to the innermost invocation frame. They drain FIFO
  before control returns to the caller, so navigation stays stack-shaped rather
  than flattening nested flows into a misleading playlist.
- Steering degrades honestly. Codex accepts live input; harnesses without that
  capability queue the message for the next body.
- The Mac reads a bounded recent-run snapshot from the local store. PR liveness
  comes from the store's synchronized PR record, not from network work in the
  UI refresh path.

## How it fits together

The listener owns the wave runtime, journal, and resident seat. The resident
runs the body selected by the playhead and streams ordered deltas back to the
runtime; the runtime journals and broadcasts a single replayable thread plus
playhead view. CLI controls mutate that runtime, while Loopflow Mac decodes the
same snapshots and event stream rather than maintaining a parallel model.

Foreground and detached child loops still receive the parent wave's memory and
recent context at each pass boundary. They report durable outcomes upward; only
promotion creates a new resident, endpoint, cadence, budget, and human thread.

## Risks and bottlenecks

- Recovery correctness depends on chat-turn and playhead-body terminal events
  staying paired. The force-finalization regression test now proves the active
  seat is released in memory and replay without advancing the interrupted step.
- `lf status` reads live state for at most 20 recent PR-bearing runs. These are
  local SQLite reads, but the sequential bounded lookup is the main added cost
  in the Mac's five-second polling path.
- The Rust/Swift status shape is hand-maintained. Required-present optional
  fields and cross-language decoding tests make drift fail loudly.
- Nothing enforces one writer per wave worktree. This branch encountered that
  race once; the unresolved policy remains recorded in `scratch/questions.md`.
- The MVP is a UI interaction sequence. This gate ran in an explicitly
  headless environment, so no screenshot could be captured. The app and UI-test
  bundles build and link, and all non-rendering Xcode tests pass; the screenshot
  runner itself waits indefinitely for a GUI worker here.

## What's not included

- The bus/thread protocol rewrite, including server-stamped attribution and a
  writes-down/reads-up capability gate.
- True mid-turn steering for Claude or OpenCode.
- Composite `and`/`or`/`xor`/`loop` nodes as first-class playhead frames.
- Provider-level removal of residual `project:<slug>` labels during promotion.
- Longer default caps for project loops.
- A foreground/background owner label in the run ledger.
- Enforcement of one writer per wave worktree.

## Validation

- Final changed-aware run: Rust 1258/1258 passed (3 configured skips), website
  59 passed (3 device-title skips), Swift 307 tests across 50 suites plus 5
  XCTest cases passed.
- Full matrix: Python 51 passed; Rust, website, Swift, and the end-to-end smoke
  suite passed.
- Xcode: clean build/link passed after clearing stale DerivedData; the
  `LoopflowTests` target passed 307 Swift Testing cases plus 5 XCTest cases.
  `LoopflowUITests/ScreenshotPipelineTests` was not executed because this run
  has no rendering worker.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
  and `git diff --check` pass.
- Focused recovery, status-wire, and Swift registry tests pass.

This advances the `wave-chat` and `loopflow-api` projects: one retained thread
now reaches the active body and exposes the same wave/playhead model in CLI and
Mac. The month-long dogfood KRs remain post-merge evidence, not something this
gate can claim. Live Linear task alignment could not be read because the stored
Linear authorization has expired.

# Minds design review

## What was implemented

This branch turns each served wave into one persistent, inspectable mind rather
than a sequence of disconnected agent runs. `lf serve <wave>` owns a durable
human thread and playhead; disposable resident bodies execute the selected flow
step, stream into that thread, and can be steered, skipped, retried, or replaced
without losing the route.

Foreground and detached work now share `lf loop <flow> <seed>`, with `--detach`
as the concurrency switch. Memory inherits down the wave chain while chat stays
local, `lf project promote` creates a resident child wave, and Loopflow Mac
renders the thread, playhead, KRs, open PRs, active sessions, and filed backlog
on one wave screen.

Agent communication is now a separate store-backed bus. `lf radio` inserts a
row without needing a server, `lf sub` polls a channel prefix, and a served mind
folds family reports into its own journal from a durable cursor. The gate fixed
two final recovery/routing defects:

- A swept-empty bus now advances the subscriber cursor when it announces the
  gap, so the same missing range is not reported again on every restart.
- Loop cap failures report with `lf radio` from the hand's placed worktree,
  instead of using the retired chat transport and accidentally interpreting
  “parent” as `parent_wave_id`.

## Key choices

- The journal is the source of truth for the human thread and playhead.
  Sessions and resident bodies are replaceable.
- Waves are the only persistent minds. Bounded loops are hands with private
  transcripts; promotion is the explicit way to create another room.
- Enqueued flows belong to the innermost invocation frame and drain FIFO before
  returning to the caller, preserving stack-shaped navigation.
- The database is the agent bus. Publishing is an INSERT and subscribing is a
  forward poll; no listener process brokers agent speech.
- Bus bylines are testimony and channels are evidence. The row preserves both
  rather than pretending client-submitted attribution is authenticated.
- Bus delivery is at-least-once. The listener journals before advancing its
  cursor because a duplicate report after a crash is cheaper than a lost one.

## How it fits together

`lf serve` starts a listener in the origin repo and a resident in the wave
worktree. The listener owns the journal, thread doors, store observers, bus
cursor, discovery files, and resident supervision; the resident owns the
playhead scheduler and disposable harness bodies. CLI and Mac consume the same
thread, playhead, registry, and status shapes.

Detached loops write PR/run records and broadcast on their own bus channels.
The served mind polls its channel family, journals one attributed copy, and
wakes the resident. Human chat stays on the listener's durable HTTP/SSE thread;
the two wires no longer share transport or semantics.

## Risks and bottlenecks

- Bus rows expire after one hour. A durable subscriber reports a cursor gap,
  but the PR and run ledger remain the records of record after expiry.
- Journaling and cursor commit are not one transaction. A crash in that seam
  can duplicate one report; it cannot silently lose it.
- Byline is client testimony, not identity proof. Readers must compare it with
  the arrival channel when provenance matters.
- A detached loop does not poll its own channel. Speaking on a hand's channel
  reaches live `lf sub` listeners, not the hand mid-pass; its reliable ear is
  still the wave thread at the next pass boundary.
- Rust and Swift status shapes are hand-maintained. Required-present optional
  fields and cross-language decoding tests make drift fail loudly.
- Nothing enforces one writer per wave worktree. This branch encountered that
  race during editing and rebasing; the store-lease fix remains separate work.

## What's not included

- True mid-turn steering for Claude or OpenCode.
- First-class playhead frames for composite `and`/`or`/`xor`/`loop` nodes.
- Provider-level removal of residual `project:<slug>` labels during promotion.
- Longer default caps for project loops.
- A persisted foreground/background owner label in the run ledger.
- A private poll cursor for detached hands or a one-writer worktree lease.
- The `lf chat` versus `lf wavechat` surface consolidation.

## Validation

- Changed-aware run: Rust 1298/1298 passed (3 configured skips), website 59
  passed (3 device-title skips), Swift 309 tests across 50 suites plus 5 XCTest
  cases passed.
- Full matrix: Python 51 passed; Rust, website, Swift, end-to-end smoke, and the
  Loopflow Xcode build-for-testing suite passed.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  the Swift multiplatform boundary guard, and `git diff --check` pass.
- Focused bus recovery and loop-driver tests pass. The new recovery assertion
  reboots an emptied bus before another publish and proves the gap appears once.
- The app, test bundle, and UI-test bundle build and link. This run is headless,
  so no rendered MVP screenshot was captured and UI automation was not launched.
- CI's `scratch-clear` job is intentionally deferred: these gate artifacts are
  inputs to `lf pr submit` / `lf pr land`, which clears them before the PR gate.

This advances the `wave-chat` and `loopflow-api` projects: the thread now
survives disposable bodies, agent reports survive a sleeping mind within the
bus window, and CLI/Mac expose the same wave/playhead model. The month-long
dogfood KRs remain post-merge evidence. Live Linear task alignment could not be
read because the stored Linear authorization has expired.

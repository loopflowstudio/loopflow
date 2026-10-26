# LOO-612 — One decoder per harness emitting ProviderFact

Status: **slices 1–3 implemented, store-free trace replay built,
`TurnStarted`/`TurnAnswered` split and `AvailabilityReading` rename done**
(2026-10-26); only the gate remains. Jack Heart accepted the direction in LOO-602 decision 7
(design: `loo-602-tracking.md` §5 in the LOO-602 checkout). Implementation
choices that carry product weight are in `questions.md`.

## Remaining work

- **Gate:** the full `cargo test -p loopflow --lib harness` run, where five
  untouched `context`/`agent_process`/`claude::activity_tests` tests failed
  under machine load but pass in isolation; then `--test integration history
  session`, and the headless Claude/Codex usage + monitor demo.

## Implemented shape (2026-10-26)

- `fact.rs` holds `ProviderTurn {thread, turn}`; `Signal` (the old tracker
  signals plus `NewTurn` and `Previewed`); and `ProviderFact`, with variants
  `SessionObserved`, `TurnStarted` (Codex `turn/started`; writes the
  `Started` receipt), `TurnAnswered` (correlated request; writes the origin),
  `Activity`, `AvailabilityReading` (Pi's extension counts its own reading), `Output`, `Usage`, `TurnCompleted` and
  `RateLimit`. It also holds `Decoded {Fact | Display}`.
- Decoders are per-connection structs with no store, route or channel:
  - `ClaudeDecoder::{outbound, decode}`.
  - `CodexDecoder::{outbound, decode}`: one decoder for the headless client,
    the relay and `thread/turns/list` backfill, all through
    `codex::CodexStream`.
  - `OpenCodeDecoder::{event, messages}`, where `messages` is the snapshot
    diff.
  - `PiDecoder::{event, finish}`, plus `pi_decode::native` for the extension
    callback, whose entry point is `pi::native_event`.
- Display lifecycle stays `Decoded::Display` and comes from the same decode
  call as the fact; it is not projected from facts. Display turns and
  provider turns differ: Claude coalesces steer turns, Codex display usage is
  baseline-delta, and snapshot completions never display (see
  `questions.md`).
- `fact_writer.rs`: `FactWriter::record` is the synchronous path for
  receipts, activity and native conversation selection. `write` is `record`
  plus `pin_session`/`record_rate_limit`.
  - `TurnAnswered{request}` uses the origin frozen at send. Otherwise it uses
    the current attachment's origin, which covers input submitted by the
    provider's own client (the OpenCode terminal, the Pi extension).
  - The writer owns the availability tracker fold and the ported store
    goldens.
- Deleted: every `*_history.rs` and `*_mapping.rs`, `activity_tracker.rs` and
  `codex::process_notification`. In `harness/`,
  `record_session_event|record_session_activity|pin_session|record_rate_limit|record_session_agent_session`
  matches only `fact_writer.rs`.
- Store-free trace replay: `trace_<name>` tests in `claude_decode.rs`,
  `codex_decode.rs` and `opencode_decode.rs` decode each `testdata/` trace
  alone (loaded through `fact::trace`, so the decoder files never name
  `testdata/`) and pin its `fact::fact_lines` sequence. The Claude replay first
  echoes a Loopflow request, since the recorded traces start after it. Codex
  emits `Output` after `TurnCompleted`; OpenCode events carry only `Activity`
  (turns come from snapshots); a Claude crash mid-tool ends with no
  completion. Pi has no recorded trace (only the `pi_rpc.py` stand-in), so
  its facts are covered only through `pi_tests.rs`.
- Receipt payloads are unchanged and there is no migration. The live
  `ConversationEvent` stream is unchanged; `conformance_tests.rs` passes.

## Demo

```
cargo test -p loopflow --lib harness::fact_writer
cargo test -p loopflow --lib _decode::tests
```

The writer tests replay the recorded traces through decoder + `FactWriter` and
assert the receipt rows and availability readings that main writes.
`_decode::tests` pins each trace's fact sequence with no store. Product-visible demo: run a Codex and a Claude headless
`lf -b skill …`, then check that `lf history usage` and `lf monitor` show the
same numbers and states as before. Nothing a person sees changes. The gain is
one place to read and fix provider parsing, which LOO-611's store work and
LOO-602 decision 3's per-request facts build on.

## Constraints that still hold

- Decoders never touch `SqliteStore`, `AccountRoute` or a channel. No native
  message is parsed twice. A turn's lifecycle reaches the store only through
  facts and reaches display only through `Decoded::Display`. Neither is
  derived from the other.
- Correlation is per provider and lives in its decoder: Claude's request UUID
  echo, Codex's `first_seen >= request` sequence fence, OpenCode's snapshot
  baseline. The decoder tracks *which* request a turn answers; the writer
  holds *what* that request's origin was.
- Write-failure policy is per kind: a receipt failure ends the reader, an
  activity or pin failure warns.
- Claude interactive sessions produce no stream facts; out of scope.
- Ordering with LOO-611: the live `ConversationEvent` sequence stays
  identical, so the two can land in either order.

## Alternatives rejected

- Pure `fn decode(&Value)` with correlation in the writer: the writer would
  grow four provider branches.
- Display derived entirely from facts: deltas, diffs and `<lf>` tags would
  become facts, and the store would bloat again (LOO-602 §5).
- Facts and display as two `Vec`s: `TurnCompleted` would exist twice and the
  two would need re-merging.

## Scope exclusions

- Slice 4, the capture owner (concept review, 2026-10-26). Accepted decision 7
  says decoders turn each *native line* into `ProviderFact`; account selection
  never appears in native output, so it is a launch fact, not a decoder fact.
  `SessionCapture::observe_provider` (`agent/mod.rs:1288,1332,1559`) and
  `route.pin_session_blocking` (`agent/mod.rs:1416`) stay as they are and
  belong to LOO-611's recorder (not on main as of 2026-10-26). No
  `AccountSelected` variant is added here.

- Recorder persistence, observed usage and retention prune (LOO-611).
- Per-request prompt-cost facts and `--events` removal (LOO-602 decision 3).
- Native transcript readers and `Turn` (LOO-602 decision 4).
- Receipt payload shapes, `session_events` schema, Desktop/Swift wire.

## Done when

- Each harness has one decoder file. No `_history`/`_mapping` file or
  `activity_tracker.rs` remains, and the store-write grep above matches only
  `fact_writer.rs`. **Met.**
- Every trace in `harness/testdata/` replays through its decoder without a
  store and yields the expected `ProviderFact` sequence. **Met** (Pi:
  `pi_tests.rs` only; no recorded trace).
- Through `FactWriter`, the traces leave main's receipt rows and availability
  readings, and `conformance_tests.rs` display assertions pass unchanged.
  **Met (writer tests).**
- Headless Claude and Codex runs report the same `lf history usage` totals and
  `lf monitor` availability as before. **Gate.**

## Acceptance at gate (headless)

With inherited `LF_*` unset and `< /dev/null`:
- `cargo test -p loopflow --lib harness`
- `cargo test -p loopflow --test integration history session`
- `cargo clippy --all-targets -- -D warnings`

Check (2026-10-26): `cargo test -p loopflow --lib _decode::tests harness::fact_writer harness::pi_tests` 77 pass (all seven traces, receipt goldens unchanged); clippy `-p loopflow --all-targets -D warnings` clean. `journal::tests::canonical_ids_journal_appends_to_retained_trace_directory` fails in isolation outside this diff; gate confirms against main.

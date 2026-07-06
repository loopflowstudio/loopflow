# Chord Played - 2026-07-06

## Source
`scratch/garden-assessment.md`

## Summary
The chord did not mutate wave config directly. The useful move is dispatch:
send small workers after the three pressure points surfaced by the scan.

## Mutations
### 1. Repair run-ledger concurrent writes
**Wave**: meta
**Lever**: agent
**Before**: Parallel `lf op pm show` commands can interleave bytes in the same
`.lf/journal/.../events.jsonl`, making the local run record unreconstructable.
**After**: Dispatch a worker to find the writer, make event appends atomic or
serialized, and add a regression test that fails on interleaved JSONL.
**Rationale**: This directly protects Meta's first metric.
**Risk**: The writer may live in shared execution plumbing; a narrow fix must not
hide process failures or slow normal runs significantly.
**Files changed**: none yet
**Status**: applied
**Notes**: `lf implement ... --wave meta --dispatch` failed because wave `meta`
was not found in the registry. Fallback sub-agent: Faraday
(`019f34fb-4ab9-7cc3-8dbf-ae2832707a4a`). Faraday changed
`rust/loopflow/src/journal/mod.rs`: event writes now serialize each event to one
buffer and guard appends with Unix `flock`; regression coverage spawns
concurrent child writers and parses every JSONL line. Parent verification:
`cargo fmt --manifest-path rust/loopflow/Cargo.toml --check` and
`cargo test --manifest-path rust/loopflow/Cargo.toml concurrent_child_process_appends_keep_events_jsonl_parseable -- --nocapture`.

### 2. Triage dirty Goals worktree
**Wave**: goals
**Lever**: agent
**Before**: `/Users/jack/src/loopflow.jack-heart.bugs.20260705_1627.goals` has a
large dirty diff mixing dispatch extraction, harness conformance, and Swift
parser deletion.
**After**: Dispatch a worker to review the dirty worktree, classify changes by
concern, and recommend or perform the minimal safe split/checkpoint without
overwriting user work.
**Rationale**: Prevents valuable branch work from becoming an unreviewable knot.
**Risk**: The worker must preserve user changes and avoid destructive git ops.
**Files changed**: none yet
**Status**: applied
**Notes**: `lf triage ... --wave goals --dispatch` failed because dispatch could
not write `.git/FETCH_HEAD` in this sandbox. Fallback sub-agent: Feynman
(`019f34fb-d21b-72c1-8c66-8c51c659d773`). Feynman wrote
`scratch/goals-worktree-triage.md`. Finding: prefer Architecture's
`rust/loopflow/src/dispatch.rs` as source of truth; the dirty Goals worktree
adds `rust/loopflow/src/dispatch/mod.rs`, which would conflict with
`pub mod dispatch` if both paths land.

### 3. Resolve wave roster truth
**Wave**: meta
**Lever**: agent
**Before**: Current local files define root/mobile/workflows wave surfaces;
`jack-heart.wave-roster-tidy.20260705_1800` deletes them and adds Concerto's
Asana mapping.
**After**: Dispatch a worker to review the roster-tidy branch against current
wave goals and report whether to land, revise, or reject it.
**Rationale**: Garden and mutation logic need one active-wave set.
**Risk**: Deleting wave surfaces before Asana roadmap access is reliable may make
status harder to reconstruct.
**Files changed**: none yet
**Status**: applied
**Notes**: `lf review-open-work ... --wave meta --dispatch` failed because wave
`meta` was not found in the registry. Fallback sub-agent: Nietzsche
(`019f34fb-e326-7282-b243-ed33e3760aed`). Nietzsche wrote
`scratch/roster-tidy-review.md`. Verdict: revise before landing; Concerto's
Asana mapping and Mobile archival cleanup are fine, but Root and Workflows
deletion needs explicit ownership migration first.

### 4. Add Concerto PM mapping
**Wave**: concerto
**Lever**: items
**Before**: `wave/concerto/GOAL.md` had no `pm.asana_project`, so `lf op pm`
could not read Concerto's live roadmap from the authored wave surface.
**After**: Added the Asana provider block with project `1214270017631632`.
**Rationale**: This is the safe subset of `wave-roster-tidy`: it improves live
roadmap access without deleting Root or Workflows ownership surfaces.
**Risk**: The Asana credential path is still blocked in this sandbox, so the
mapping could not be live-verified here.
**Files changed**: `wave/concerto/GOAL.md`
**Status**: applied
**Notes**: Source was `jack-heart.wave-roster-tidy.20260705_1800`; Root and
Workflows deletions are deferred pending explicit ownership migration.
Runtime sync was attempted with `lf op update-wave --wave concerto`, but this
installed `lf` does not expose that subcommand.

## Deferred
- Live Asana roadmap and GitHub PR status verification are deferred because this
  sandbox cannot access keychain-protected Asana credentials or GitHub.
- Website work stays quiet; no current evidence shows it is the highest-leverage
  Meta move.

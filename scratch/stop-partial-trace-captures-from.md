# Stop Partial Trace Captures from Accumulating

## Problem

`telemetry-daily` is permanently red even though the retained evidence is mostly
intact. On 2026-07-18, published revision `1a3079a94` reported 787 capture
failures across 2,262 launches and 2,272 turns. The 2026-07-19 daily run still
reported exactly 787 failures after the ledger grew to 2,381 launches and 2,391
turns. Those 119 new launches and 119 new turns added zero failures. This is
strong forward evidence that current writes are healthy and the remaining
defect is accumulated terminal residue, reconciliation, or classification.

The exact 787 failures are:

| Failure shape | Count | Creation time (UTC) | Outcome / provider | Artifact state |
|---|---:|---|---|---|
| Launch row explicitly `partial` | 10 | Jul 15: 5; Jul 16: 4; Jul 17: 1 | All recorded `completed`; Claude: 1, Codex: 9 | Conversation, turn, and prompt artifacts remain. Every `incomplete_reason` is an `ENOSPC` write failure. |
| Artifact directory with no launch row | 777 | Jul 14: 24; Jul 15: 118; Jul 16: 42; Jul 17: 18; Jul 18: 575 | The owning store row is gone, so terminal outcome and provider are irrecoverable for 739. Surviving raw evidence identifies 7 Claude-completed, 21 Codex-completed, 1 Claude-unknown, and 9 Codex-unknown captures. | All 777 have conversation and turn artifacts; 760 also have provider events. No retained launch points at a missing directory. |

The orphan directories are the post-ENG-85 path. Older development binaries
wrote private-store captures under the shared trace root. Once the private
store disappeared during terminal cleanup, those artifacts remained visible to
the production doctor's reverse-reference scan. Current main already fixes new
creation by deriving `trace_root()` from the same Home resolver as the store;
its focused test passes. The published binary is stale, but freshness is not
the repair: current main still reports the same 787 failures on a copied
production store.

The shipped reconciliation also creates new permanent failures. On a copy,
`lf runs reconcile --all --apply` removed the 777 orphan directories but changed
78 stale, intact `capturing` launches to `partial` while leaving their launch
outcome `running` and their current turns nonterminal. Doctor then failed with
88 partial launches: the original 10 plus the newly manufactured 78. A second
reconciliation was a no-op. The operation is mechanically idempotent but
semantically wrong.

This taxes the Developer Efficiency KRs directly: an operator cannot tell fresh
loss from dead residue, and cleanup requires repeated human interpretation. The
design advances “avoidable human-in-the-loop setup or repair steps ... fall to
zero” and “no Task strands on a dead body.”

## The demo

Against a copy-on-write clone of the production Home, run
`lf runs reconcile --all --apply`, run it a second time, then run `lf doctor`.
The first pass reports 777 orphan artifacts removed and 88 launch captures
tombstoned with reasons; the second reports zero changes; doctor reports zero
capture failures and exits zero. A fixture that interrupts an intact
`capturing` launch shows launch outcome `interrupted`, its running turn
`interrupted`, and capture status `pruned` rather than `partial`.

## Approach

Keep the capture lifecycle in the existing ledger and make reconciliation an
explicit terminal transition. Do not change provider capture writing unless a
fresh completed/failed/interrupted fixture against the copied current-main Home
increases the capture-failure count; the observed default hypothesis is that
the writer is already healthy.

1. Broaden `pruned` from “artifact known absent” to “capture is explicitly
   terminal and its incomplete evidence is acknowledged.” It remains the
   single tombstone state and always carries `incomplete_reason`. Doctor counts
   tombstones but does not treat their acknowledged incompleteness as fresh
   loss.
2. Replace the generic launch-status setter with operations matching the two
   real transitions:
   - tombstone an old `partial` launch or a terminal launch whose artifact is
     missing;
   - interrupt a dead/stale `capturing` launch, atomically setting launch
     `capture_status = 'pruned'`, launch `outcome = 'interrupted'`, launch
     `ended_at`, and every running turn's status/`ended_at` to `interrupted`.
3. Extend the pure reconciliation plan so old `partial` rows are tombstone
   candidates, recent partials remain visible as fresh loss, and dead/stale
   `capturing` rows plan the interrupted transition instead of a new partial.
   Preserve the 48-hour guard; `--all` remains the explicit override for copied
   historical state.
4. Keep orphan cleanup in the existing trace-root reconciliation. Doctor must
   tolerate a newly created unclaimed directory inside the same age guard
   because `CaptureHandle::begin` creates the directory before inserting the
   row. Aged unclaimed directories remain failures until acknowledged cleanup.
5. Preserve current main's per-Home trace placement. Do not add another cleanup
   hook to terminal worktree pruning: current placement keeps new private trace
   artifacts out of the production root, and reconciliation owns old shared
   residue.
6. Repair pending migration `0.11.031_durable_input_spine` before promotion.
   It currently copies `provider_session_id` into `resume_token` on 1,749 legacy
   trace launches that have no product Run/Home/containment metadata; the main
   reader then rejects the lone token as an incomplete control Launch. Restrict
   that backfill to actual control launches. This is a correction to an
   unpublished migration, not a compatibility reader for two schemas.

The live `~/.lf/loopflow.db` and trace root are never mutation targets during
implementation or proof. SQLite backup plus copy-on-write trace clones provide
the production shape; all reconciliation writes and artifact deletions stay in
the clone.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Are the 787 failures fresh partial captures? | No. They are exactly 10 disk-full partial rows plus 777 intact orphan directories. From the 2026-07-18 to 2026-07-19 daily run, launches and turns each grew by 119 while the failure count stayed exactly 787. | Repair historical classification and terminal cleanup. Treat provider-writer changes as disproven unless copied-state lifecycle proof produces a new failure. |
| Did a merged change after `1a3079a94` already fix the issue? | Partly. Current main's Home-root test proves the cross-store artifact split is closed for new builds, but current main still reports 787 on the copied store. | Keep the placement fix and narrow implementation to reconciliation, doctor classification, and migration proof. |
| Is current reconciliation safe and convergent? | Orphan deletion converges, but stale intact captures are finalized as `partial`; 78 were created on the first pass and the second pass merely preserved the wrong state. | Make finalization a terminal interrupted transition and prove state, not only a zero-change second pass. |
| Can doctor simply ignore partial rows? | No. The 10 rows record genuine write loss, and a new partial is the signal doctor must retain. | Only an explicit age-guarded reconciliation turns partial into a non-failing tombstone. |
| Can terminal outcome/provider be recovered for all 777 orphans? | No. Pruning removed the owning private stores. Raw artifacts identify 38; 739 lack a unique provider/outcome signal. | Report the unknown partition honestly. Forward correctness comes from keeping artifacts with their owning Home, not inventing provenance. |
| Can current main run directly on the production-shaped copy? | Not without fixing the pending migration: 1,749 legacy launches receive a resume token without the rest of control Launch metadata and fail deserialization. Normalizing those tokens on a disposable copy exposed the unchanged 787 capture result. | Correct the pending migration in this PR so copied-store proof exercises current main without manual data repair. |
| Could `--all` race a capture being created? | Yes. The directory exists briefly before its row. The default 48-hour guard is the production safety boundary; `--all` is appropriate only on a quiescent clone or as an explicit operator override. | Keep the guard, teach doctor the same recent-orphan boundary, and use `--all` only in copied-state proof. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Make doctor warn on every `partial` or orphan | Immediately greens telemetry. | Masks genuine new capture loss and leaves dead rows/artifacts accumulating. |
| Delete all partial rows and orphan directories automatically from doctor | Removes manual cleanup. | Makes a read-only health command destructive, races `begin`, and erases the evidence needed to diagnose fresh loss. |
| Rebuild orphan launch rows by parsing artifacts or add a parallel telemetry store | Could restore some attribution. | Outcome/provider are irrecoverable for 739 artifacts, and a second store recreates the ownership split that caused the regression. |
| Add cleanup directly to worktree pruning | Couples worktree lifecycle to trace internals. | Current per-Home placement already isolates new private artifacts from the production doctor; it does nothing for historical shared residue and duplicates reconciliation ownership. |

## Key decisions

- `pruned` is the one acknowledged terminal tombstone. Adding another status
  would create a distinction doctor and every DTO would have to preserve with no
  behavioral payoff.
- Reconciliation updates launch and turn terminality in one SQLite transaction.
  A terminal launch over a running turn is another stranded dead body.
- The 48-hour guard applies to partial rows and reverse-reference orphans. Fresh
  loss stays red; fresh directory-before-row races stay invisible.
- Existing `incomplete_reason` is preserved inside the reconciliation reason so
  the tombstone remains actionable (`ENOSPC`, missing artifact, or interrupted
  owner), rather than becoming a generic “cleaned up” receipt.
- The migration fix changes the unpublished canonical SQL. It does not teach
  production readers to accept half a control Launch.

Wild success is boring: telemetry stays green for normal failed/interrupted
runs, a red capture check always points to new loss, and cleanup needs one
explicit command at most. Wild failure is a reconciliation that greens doctor
by erasing fresh evidence or leaves launch/turn states contradictory. The age
guard, transactional transition, and copied-store before/after proof are the
lines against that failure.

## Scope

- In scope: capture reconciliation planning; transactional launch/turn terminal
  transitions; `pruned` semantics and reasons; doctor classification for recent
  versus aged orphans; correction of the pending control-Launch backfill;
  focused tests and copied production-state proof.
- Out of scope: a parallel telemetry store; new retention daemon; provider
  transcript parsing; reconstructing deleted private-store provenance; changing
  worktree pruning; changing trace retention duration; touching the live store;
  generic multi-product infrastructure.

## Done when

- A focused lifecycle test drives an intact `capturing` launch whose process
  ends, applies reconciliation, and observes a `pruned`/`interrupted` launch and
  interrupted turn with an actionable reason. Doctor is green afterward.
- Fresh completed, failed, and interrupted launches against the copied
  current-main Home produce zero new capture failures. If that proof holds, no
  provider capture-writer code changes in this PR.
- A fresh `partial` launch still fails doctor before the 48-hour guard; an aged
  partial is tombstoned only by reconciliation (or `--all`).
- A fresh unclaimed directory is tolerated; an aged one is reported and removed
  by reconciliation.
- Current main opens and migrates an untouched production-shaped database copy
  without the incomplete-control-Launch error.
- On cloned production state, published `1a3079a94` records the 787-failure
  baseline. The fixed binary's first `lf runs reconcile --all --apply` removes
  777 orphans and terminalizes 88 captures; a second identical command reports
  zero changes; `lf doctor` reports zero capture failures and exits zero.
- The live database inode, size, modification time, and migration frontier are
  unchanged by the proof.
- `cargo fmt`, focused Rust tests, and `cargo clippy -- -D warnings` pass.

## Measure

Baseline on the 2026-07-18 copied Home:

- Doctor capture failures: 787.
- Orphan trace directories: 777.
- Existing partial launch rows: 10.
- Stale intact `capturing` launches the old reconciliation would turn partial:
  78 (38 Claude, 40 Codex; all outcome `running`, all conversations present).
- Old reconciliation after one pass: 88 partial failures; after two passes: 88.

Longitudinal production signal, read only:

- 2026-07-18: 787 failures / 2,262 launches / 2,272 turns.
- 2026-07-19: 787 failures / 2,381 launches / 2,391 turns.
- Failure growth across 119 new launches and turns: 0.

Target:

- Doctor capture failures after acknowledged reconciliation: 0.
- Reconciliation mutations on the second pass: 0.
- New partial rows produced by failed/interrupted lifecycle reconciliation: 0.
- Unacknowledged fresh-loss fixtures detected by doctor: 100%.

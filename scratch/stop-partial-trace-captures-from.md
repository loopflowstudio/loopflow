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

## User-visible outcome

An operator sees a red capture check only for evidence that is newly missing,
still partial, structurally invalid, or otherwise unacknowledged. Historical
absence is visible as `pruned`, a dead owner with retained evidence is visible
as `interrupted`, and acknowledged write loss with retained evidence is visible
as `lost`; each state keeps an actionable reason. A single explicit reconcile
converges the historical store, and normal completed, failed, and interrupted
runs add no new capture failures.

## End-to-end proof

Create two private Homes from one SQLite online backup and copy-on-write clone
of the production trace root. The published `1a3079a94` binary records the
787-failure baseline against one clone. Against the other, the fixed binary
migrates the untouched copied store, then runs
`lf runs reconcile --all --apply --json` twice and `lf doctor --json`. The first
reconcile removes 777 unclaimed directories, interrupts 78 intact stale
captures, marks 10 intact ENOSPC partials lost, and prunes zero intact captures.
The second reports zero transitions and zero removals. Doctor reports 78
interrupted, 10 lost, zero capture failures, and exits zero. Direct queries of
the copied `agent_launches` and `agent_turns` prove the terminal state and reason
invariants; each command's store report names the copied Home, never
`~/.lf/loopflow.db`.

## Approach

Keep the capture lifecycle in the existing ledger and make reconciliation an
explicit terminal transition. Do not change provider capture writing unless a
fresh completed/failed/interrupted fixture against the copied current-main Home
increases the capture-failure count; the observed default hypothesis is that
the writer is already healthy.

1. Add two precise terminal capture statuses in
   `0.11.037_capture_terminal_states`:
   - `interrupted`: referenced artifacts remain present, but the owning process
     ended before capture finalization. Launch outcome is `interrupted`, every
     formerly running turn is `interrupted`, and `incomplete_reason` names the
     dead-owner reconciliation.
   - `lost`: referenced artifacts remain present, but a recorded capture
     write/sync failure means the evidence has a known gap. This is the explicit
     operator acknowledgment of an aged `partial`; its original failure reason
     remains in `incomplete_reason`.
   `pruned` keeps its existing invariant: a referenced conversation artifact is
   known absent. Doctor skips artifact resolution only for `pruned`.
   The migration rebuilds `agent_launches` with the widened CHECK exactly as
   `capture_pruned_state` did, preserves every row and launch index, and extends
   the migration fixture to prove all seven statuses at the copied store frontier.
2. Replace the generic launch-status setter with operations matching the three
   real transitions:
   - mark a terminal capture `pruned` only when its referenced artifact is
     absent;
   - interrupt a dead/stale `capturing` launch with intact artifacts,
     atomically updating launch outcome/status/end time and every running turn;
   - acknowledge an aged intact `partial` launch as `lost` while preserving its
     original terminal outcome, artifact references, and failure reason.
3. Extend the pure reconciliation plan so aged intact `partial` rows become
   `lost` candidates, recent partials remain `partial` and red, and dead/stale
   intact `capturing` rows become `interrupted`. A later missing artifact may
   move `complete`, `partial`, `prompt_only`, `interrupted`, or `lost` to
   `pruned`, but no intact capture can enter `pruned`. Preserve the 48-hour
   guard; `--all` remains the explicit override for copied historical state.
4. Make doctor report the state machine directly. `partial` and a capture left
   `capturing` after its owner ended are failures. `interrupted` and `lost` are
   acknowledged terminal counts whose retained artifact paths must still
   resolve. `pruned` is an acknowledged terminal count with a required reason
   and known-absent conversation artifact. Missing evidence from any intact
   state remains a fresh failure until reconciliation records `pruned`.
5. Keep orphan cleanup in the existing trace-root reconciliation. Doctor must
   tolerate a newly created unclaimed directory inside the same age guard
   because `CaptureHandle::begin` creates the directory before inserting the
   row. Aged unclaimed directories remain failures until acknowledged cleanup.
6. Preserve current main's per-Home trace placement. Do not add another cleanup
   hook to terminal worktree pruning: current placement keeps new private trace
   artifacts out of the production root, and reconciliation owns old shared
   residue.
7. Repair the resume-token backfill in pending durable-input follow-on
   `0.11.032_run_launch_attention` before promotion. It currently copies
   `provider_session_id` into `resume_token` on 1,749 legacy trace launches that
   have no product Run/Home/containment metadata; the main reader then rejects
   the lone token as an incomplete control Launch. Restrict that one statement
   to rows already carrying product Run authority. This correction stays
   limited to the demonstrated blocker to opening the untouched copy on current
   main; it does not change capture lifecycle semantics or add compatibility
   parsing.

The live `~/.lf/loopflow.db` and trace root are never mutation targets during
implementation or proof. SQLite backup plus copy-on-write trace clones provide
the production shape; all reconciliation writes and artifact deletions stay in
the clone.

## Source of truth

`agent_launches` is authoritative for capture status, terminal outcome,
end time, artifact references, and `incomplete_reason`. `agent_turns` is
authoritative for each Turn's terminality. The Home-scoped trace root is
authoritative only for whether referenced evidence exists; filesystem presence
does not invent or change ledger state. `lf runs reconcile` is the sole writer
for historical acknowledgment transitions, and its SQLite transaction keeps a
Launch and its running Turns consistent. `lf doctor`, `lf runs`, context views,
and `telemetry-daily` derive their presentation and exit status from those
records plus read-only artifact validation.

## Affected surfaces and consumers

- The SQLite migration chain widens the closed `capture_status` CHECK and
  preserves every `agent_launches` row, foreign key, and index. The pending
  resume-token backfill remains the only adjacent migration edit.
- `lf runs reconcile` planning, text output, and JSON output report `pruned`,
  `interrupted`, `lost`, recent missing references, and removed orphan
  directories separately. A second identical run is empty.
- `lf doctor` text and JSON checks count the three acknowledged terminal states,
  validate every intact state's retained artifacts, keep fresh `partial` red,
  and ignore only recent directory-before-row races. `telemetry-daily` consumes
  this exit status without a new telemetry store or exception list.
- `lf context` capture-state validation accepts the widened state set; generic
  `lf runs`, trace, usage, and receipt readers continue carrying the stored
  status string without changing their wire DTOs. No Swift or app DTO models
  mirror this database enum.

## Absent and error states

- A missing conversation from any intact state is a doctor failure until an
  age-guarded reconcile explicitly records `pruned`; an unsafe artifact path is
  always a failure and is never acknowledged.
- `pruned` without a reason violates the ledger invariant. `interrupted` or
  `lost` without their referenced retained files also fails doctor. A `partial`
  without an actionable reason remains `partial` and red rather than being
  silently acknowledged as `lost`.
- A fresh unclaimed directory is ignored for 48 hours because capture creation
  publishes the directory before its launch row. An aged unclaimed directory
  is red until reconcile removes it; `--all` is an explicit quiescent-clone
  override.
- Each Launch/Turn transition commits atomically. A stale plan that no longer
  matches the stored source state fails without changing either record. Orphan
  deletion errors stop the command and remain retryable; the next plan derives
  only work that still exists.
- Migration or copied-Home open failure is surfaced directly. The reader does
  not accept a resume token as a substitute for missing product Run, Home, or
  containment authority.

## Operational boundary

Reconciliation is a local, network-free pass over the Home's launch ledger,
run events, and trace-directory metadata. It preserves the 48-hour default
guard and performs no provider subprocess or transcript reconstruction. The
production-shaped proof runs only with `LF_HOME` and `LF_DB_PATH` resolved
inside a disposable clone; command-reported paths are asserted before any
`--apply`. The live database may continue changing under normal Loopflow work,
so proof of isolation is target-path containment rather than comparing its
incidental size or modification time.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Are the 787 failures fresh partial captures? | No. They are exactly 10 disk-full partial rows plus 777 intact orphan directories. From the 2026-07-18 to 2026-07-19 daily run, launches and turns each grew by 119 while the failure count stayed exactly 787. | Repair historical classification and terminal cleanup. Treat provider-writer changes as disproven unless copied-state lifecycle proof produces a new failure. |
| Did a merged change after `1a3079a94` already fix the issue? | Partly. Current main's Home-root test proves the cross-store artifact split is closed for new builds, but current main still reports 787 on the copied store. | Keep the placement fix and narrow implementation to reconciliation, doctor classification, and migration proof. |
| Is current reconciliation safe and convergent? | Orphan deletion converges, but stale intact captures are finalized as `partial`; 78 were created on the first pass and the second pass merely preserved the wrong state. | Transition those 78 to explicit `interrupted` state atomically and prove state, not only a zero-change second pass. |
| Can doctor simply ignore partial rows? | No. The 10 rows record genuine write loss, and a new partial is the signal doctor must retain. | Only explicit age-guarded reconciliation moves an intact partial to the distinct non-failing `lost` state. |
| Can terminal outcome/provider be recovered for all 777 orphans? | No. Pruning removed the owning private stores. Raw artifacts identify 38; 739 lack a unique provider/outcome signal. | Report the unknown partition honestly. Forward correctness comes from keeping artifacts with their owning Home, not inventing provenance. |
| Can current main run directly on the production-shaped copy? | Not without fixing the pending migration: 1,749 legacy launches receive a resume token without the rest of control Launch metadata and fail deserialization. Normalizing those tokens on a disposable copy exposed the unchanged 787 capture result. | Correct the pending migration in this PR so copied-store proof exercises current main without manual data repair. |
| Could `--all` race a capture being created? | Yes. The directory exists briefly before its row. The default 48-hour guard is the production safety boundary; `--all` is appropriate only on a quiescent clone or as an explicit operator override. | Keep the guard, teach doctor the same recent-orphan boundary, and use `--all` only in copied-state proof. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Make doctor warn on every `partial` or orphan | Immediately greens telemetry. | Masks genuine new capture loss and leaves dead rows/artifacts accumulating. |
| Reuse `pruned` for every acknowledged incomplete capture | Avoids one schema state. | Violates `capture_pruned_state`: doctor skips artifact resolution because `pruned` means the referenced conversation is known absent. It would mislabel intact evidence and weaken health checks. |
| Reuse `interrupted` for acknowledged ENOSPC captures | Avoids a separate intact-loss state. | The 10 launches completed and their provider owner did not interrupt; the capture writer lost evidence. Calling them interrupted destroys the causal distinction doctor needs. |
| Delete all partial rows and orphan directories automatically from doctor | Removes manual cleanup. | Makes a read-only health command destructive, races `begin`, and erases the evidence needed to diagnose fresh loss. |
| Rebuild orphan launch rows by parsing artifacts or add a parallel telemetry store | Could restore some attribution. | Outcome/provider are irrecoverable for 739 artifacts, and a second store recreates the ownership split that caused the regression. |
| Add cleanup directly to worktree pruning | Couples worktree lifecycle to trace internals. | Current per-Home placement already isolates new private artifacts from the production doctor; it does nothing for historical shared residue and duplicates reconciliation ownership. |

## State machine

| Current state | Evidence invariant | Reconciliation transition | Doctor |
|---|---|---|---|
| `capturing` | Owner may still write; the artifact directory is allocated and references may still be mid-publication. | If owner is dead or stale and artifacts remain: `interrupted`. If the conversation is absent and the owner is dead/stale: `pruned`. | Fail only when the owner is known ended; otherwise in progress. |
| `complete` / `prompt_only` | Capture finalized consistently. | If the referenced conversation later disappears: `pruned`. | Resolve and validate retained artifacts. |
| `partial` | Fresh, unacknowledged capture write/sync loss; retained artifacts may have a gap. | If aged or `--all` and artifacts remain: `lost`. If the conversation is absent: `pruned`. | Fail and show the original reason. |
| `interrupted` | Artifacts remain; owner and formerly running turns are atomically interrupted. | If the conversation later disappears: `pruned`. | Count as acknowledged terminal; resolve retained artifacts. |
| `lost` | Artifacts remain; an aged write/sync gap is explicitly acknowledged. | If the conversation later disappears: `pruned`. | Count as acknowledged terminal; resolve retained artifacts and retain the loss reason. |
| `pruned` | Referenced conversation artifact is known absent and reason is recorded. | None. | Count as acknowledged terminal; skip artifact resolution under the absent-reference invariant. |

## Key decisions

- `pruned`, `interrupted`, and `lost` are distinct because absence, owner death,
  and write loss demand different validation and operator action. The schema
  CHECK, reconciliation report, and doctor output name all three.
- Reconciliation updates launch and turn terminality in one SQLite transaction.
  A terminal launch over a running turn is another stranded dead body.
- The 48-hour guard applies to partial rows and reverse-reference orphans. Fresh
  loss stays red; fresh directory-before-row races stay invisible.
- Existing `incomplete_reason` is preserved inside the reconciliation reason so
  `lost`, `pruned`, and `interrupted` remain actionable rather than becoming
  generic “cleaned up” receipts.
- The migration fix changes the unpublished canonical SQL. It does not teach
  production readers to accept half a control Launch.

Wild success is boring: telemetry stays green for normal failed/interrupted
runs, a red capture check always points to new loss, and cleanup needs one
explicit command at most. Wild failure is a reconciliation that greens doctor
by erasing fresh evidence or leaves launch/turn states contradictory. The age
guard, transactional transition, and copied-store before/after proof are the
lines against that failure.

## Scope and exclusions

- In scope: capture reconciliation planning; transactional launch/turn terminal
  transitions; a schema migration adding `interrupted` and `lost` while
  preserving `pruned`; doctor reporting/validation for all capture states;
  recent-versus-aged orphan classification; the narrowly demonstrated pending
  control-Launch backfill correction; focused tests and copied production-state
  proof.
- Out of scope: a parallel telemetry store; new retention daemon; provider
  transcript parsing; reconstructing deleted private-store provenance; changing
  worktree pruning; changing trace retention duration; touching the live store;
  generic multi-product infrastructure.

## Done when

- A focused lifecycle test drives an intact `capturing` launch whose process
  ends, applies reconciliation, and observes capture status `interrupted`,
  launch outcome `interrupted`, and every formerly running turn `interrupted`
  with one actionable reason. Its retained artifacts still resolve and doctor
  is green afterward.
- Fresh completed, failed, and interrupted launches against the copied
  current-main Home produce zero new capture failures. If that proof holds, no
  provider capture-writer code changes in this PR.
- A fresh `partial` launch still fails doctor before the 48-hour guard. An aged
  intact partial becomes `lost` only through reconciliation (or `--all`), keeps
  its ENOSPC reason and artifact references, and is reported separately from
  both `interrupted` and `pruned`.
- A terminal capture with a removed conversation becomes `pruned`; doctor skips
  artifact resolution only for that absent-reference state. Fixtures prove
  intact `interrupted` and `lost` captures can never be planned or stored as
  `pruned`.
- Migration fixtures prove `capturing`, `complete`, `partial`, `prompt_only`,
  `interrupted`, `lost`, and `pruned` survive the table rebuild with every
  launch index restored. Doctor fixtures report fresh partial, interrupted,
  lost, and pruned independently and validate retained artifacts for every
  state except the absent-reference `pruned` state.
- A fresh unclaimed directory is tolerated; an aged one is reported and removed
  by reconciliation.
- Current main opens and migrates an untouched production-shaped database copy
  without the incomplete-control-Launch error.
- On cloned production state, published `1a3079a94` records the 787-failure
  baseline. The fixed binary's first `lf runs reconcile --all --apply` removes
  777 unclaimed directories, transitions 78 intact stale captures to
  `interrupted`, and transitions 10 acknowledged intact ENOSPC partials to
  `lost`. It transitions zero intact captures to `pruned`. A second identical
  command reports zero changes; `lf doctor` reports 78 interrupted, 10 lost,
  zero capture failures, and exits zero.
- Every mutating proof command reports a database and trace root inside the
  disposable Home; no fixed-binary command with `--apply` targets the live
  database or trace root.
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
- Unclaimed directories after reconciliation: 0 (777 removed).
- Intact interrupted captures after reconciliation: 78.
- Intact acknowledged-loss captures after reconciliation: 10.
- Intact captures mislabeled `pruned`: 0.
- Reconciliation mutations on the second pass: 0.
- New partial rows produced by failed/interrupted lifecycle reconciliation: 0.
- Unacknowledged fresh-loss fixtures detected by doctor: 100%.

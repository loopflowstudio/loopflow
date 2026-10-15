# Responsive Workflow and lifecycle feedback

Draft refinement, October 15, 2026. Placement: Product, existing Task LOO-462,
[PR #1531](https://github.com/loopflowstudio/loopflow/pull/1531).
Accepted direction remains binding; the progress presentation below is proposed.

## What to build

Make opening a Task immediately preserve its usable Workflow and workspace, and
make lifecycle actions explain whether the command is running or its result is
awaiting observation.

Jack Heart requires “fixing slow Workflow loading BEFORE landing.” The original
goal remains a work-first home for repo and Task conversations. Database reactivity,
stable rendering and semantic colors are implemented; this design closes remaining
loading and lifecycle-feedback gaps in the existing PR. Landing remains held.

## The demo

Open the configured repo conversation, select a Task with substantial execution
history, switch away and Back. Cached Workflow stays visible; fresh data replaces
it without resetting its Session, native surface, files or drafts. Expand Debug
to inspect complete history; collapse it to stop history queries.

In an isolated fixture, complete and reopen a Task through the real CLI while its
workspace is selected. The lifecycle header updates, the selected terminal Task
remains in the sidebar, navigation away removes that exception, and Back restores
the same workspace. Exercise Desktop actions with independently delayed command
completion and stream delivery. A slow observation must not look like a command
that is still executing. Real Task completion is unnecessary for this proof.

## Current system

Rust owns internal-schema reads. `SqliteStore::task_overview` omits diagnostic
Process history; `work_watch.rs` sends the lightweight Task part and separately
subscribes to `task_history`. Projections share a pinned SQLite snapshot.

Swift `WorkModel.performTaskAction` awaits the command and then
`confirmWorkUpdate(after:)`. That confirmation waits for the requested checkpoint
after dependent frames, including unchanged results. `TaskRunDraft.acting` spans
both waits; `TaskWorkflowHeader` displays “Updating Task…” throughout.

The history cut already reduced measured scope-to-frame time from roughly
0.8 seconds to 6–86 ms on the sampled Tasks. These are stream measurements,
not native rendering or input latency. The parallel
[completion investigation](slow-completion-investigation.md) records a roughly
1.2-second command and reproduces confirmation remaining busy beyond 11 seconds
despite heartbeats. The original UI delay's duration and cause remain unknown.
[Existing evidence](workflow-loading.md) retains prior checks and review feedback.

## Data structures and key functions

- Keep `TaskOverview`, complete `TaskWork`, `WorkReading`, Work frames and their
  existing Rust/Swift ownership. No schema or wire-format change is proposed.
- Replace the draft's stored `acting: Bool` with optional local
  `TaskActionPhase` (`runningCommand`, `awaitingObservation`); derive `acting`
  from phase presence. Keep errors and action ownership on the originating Task.
- `performTaskAction(_ taskId: String, _ action: () async throws -> Void) async`
  sets each phase around the existing awaits and clears it on success or failure.
- `confirmWorkUpdate(after home: String?) async throws` retains its contract.
  Reader loss or Machine changes produce an observation error; an accepted
  command is never automatically reissued.
- Give each checkpoint waiter a request-age deadline, independent of heartbeat
  liveness. Proposed starting policy: ten seconds, matching the existing transport
  silence allowance without treating it as a performance target. Expiry removes
  and settles only that waiter; late acknowledgments cannot settle it twice.
  Task action feedback says “Task update accepted; refresh not confirmed” and
  permits observation recovery without repeating the mutation. Preserve distinct
  command-failure versus confirmation-failure wording on every error path.
- Extend existing `Perf` instrumentation with separate command and confirmation
  intervals correlated to the originating action. Retain frame decode/application
  intervals and the explicitly labeled Workflow observation proxy.

## Delete — do not maintain

Replace `TaskRunDraft.acting` storage in `WorkProjection.swift` and the single
progress label in `TaskWorkView.swift` in the same cut. Migrate their constructors
and assertions; preserve `WorkModelTests` checkpoint, unchanged-result, failed-read
and navigation ownership coverage. No entire suite or historical data is deleted.
Replace unbounded checkpoint continuations with the same waiters owning their
deadline and cleanup; do not add a parallel confirmation mechanism.

The eager diagnostic-history dependency is already removed. Do not restore it
while investigating delays. Full CLI history and explicit Debug history survive.

## Constraints and forbidden outcomes

Keep Rust as sole schema reader and subscriptions as the update owner. No broad
post-write refresh, optimistic completion, second observer, or timeout that claims
success. A heartbeat or unrelated frame cannot confirm a mutation. Failures retain
last-good content and explain uncertainty. Navigation cannot terminate clients,
discard drafts, or reopen execution. Completed and Canceled remain separate from
Workflow position. Never present a main-queue callback as compositor proof.

## Internal slices

This is one coherent repair in LOO-462's existing PR, implemented in internal
slices rather than separate deliveries.

1. **This slice:** replace ambiguous action progress, bound confirmation, and
   measure command versus confirmation using deterministic delayed fixtures.
   Labels: “Updating Task…” while running; “Waiting for updated Task…” after
   command success. Exercise expiry with continuing heartbeats, a late checkpoint,
   navigation and reader loss; test elapsed-time behavior without a real ten-second
   sleep or production abstractions added solely for testing.
2. Repair the measured bottleneck in its existing owner. No speculative timeout,
   index, or subscription redesign before that evidence; update this draft with
   concrete removal targets if the cause requires an architectural change.
3. Verify cached/fresh selection, Debug disclosure and lifecycle navigation
   together. Native demo judgment remains distinct from unattended gate results.

## Done when

`scripts/test_desktop.sh --filter 'WorkModelTests|WorkNavigationTests|DesktopHeadlessTests|TaskFlowProofTests|WorkCLISubscriptionTests' -Xswiftc -gnone`
builds the app and passes. Tests distinguish the two action phases, retain errors
and workspace identity, and reject unrelated confirmation. Extend existing fixtures
rather than asserting SwiftUI body counts or private scheduling.

For a Rust repair, `uv run python scripts/test.py --rust` supplies the repository's
Rust checks; fixture commands clear inherited `LF_*` and close stdin. Gate runs
affected suites once against final content.

## Measure and open assumptions

Record command duration, confirmation duration, scope-to-frame delivery, decode,
application and Workflow observation separately. Compare identical scenarios and
binaries. The proposed confirmation deadline and progress wording are unreviewed;
no numeric latency threshold has been accepted. Native rendering and
agent-input responsiveness remain separate proof obligations.

Portfolio naming/removal ambiguity and Etude hook failures remain recorded in
the evidence notes; they are separate investigations, not inferred prerequisites
or silently completed work. No new Tasks or execution are launched by this draft.

Check: source/evidence review and `git diff --check` only; prose-only design pass,
no implementation tests rerun.

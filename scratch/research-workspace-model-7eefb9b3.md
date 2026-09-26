# Research: one outline, shared panes, Sessions and Runs

2026-09-24. Source review of the current LOO-291 working tree and LOO-293
(`loopflow.restore-task-watching-with-live`). This review precedes integration.
It supersedes the earlier assumption that Watch should be folded in unchanged.
No branch integration, Task closure, worktree removal or provider action occurred.

## System understanding

### Result and human requirements

The natural model is one planning outline plus a workspace of typed panes.
Sessions are human-interaction surfaces associated with Runs. Monitor is a
presentation of Run observations; it is not a new durable object. Flow history
and output are related details, fetched independently of the live Run list.

The human explicitly requires **Session.run_id is required**. Using the Run ID
as the Session ID is acceptable, but not required. Readable labels need not be
identifiers. This is a target contract, not a description of today's API.

The UI presents repository → Wave → Project → Task → Session once. Compact and
Sessions-only presentations promote existing nodes without changing their
identity. Monitor initially shows active Runs of the selected Task. Monitor,
Sessions and companion shells use the existing multiplexer. Two measured
experiences remain hierarchy navigation and Task workspace opening/switching.

The finish for this review is an evidenced model/API recommendation, including
migration seams and falsifying cases. A renamed Watch view, a fixture screenshot,
or a claimed live Run inferred from a Flow stage would not establish it.

### Current authorities and read contracts

| Concept | Existing authority / API | What it establishes | What it does not establish |
|---|---|---|---|
| Planning | `RoadmapSnapshot`, `RoadmapProject`, `RoadmapTask`; `lf roadmap` | PM identity, hierarchy, directive, optional runtime Work, workspace references, conditions and legal Task actions | A provider is alive or a terminal is locally attached |
| Work identity | Rust `WorkRef`, `TaskId`, `ProjectId`; stored ancestry | Durable subject and placement | Planning identity before Work exists |
| Run | `RunManifest`, `RunSnapshot`; `lf runs`, `lf usage` | Harness launch provenance, parent Run, subject selectors, recorded outcome and usage | Absence of terminal receipt does not prove liveness |
| Session | `SessionRecord`; `lf session list/open/complete/approve/iterate` | Human boundary, shared legal actions, exact opening operation, Work attribution, actual terminal attachment | Public DTO currently has no Run reference for Ask/Flow |
| Live process observation | `ActivitySnapshot`; `lf ps` | Receipt-backed OS-live Exec/provider trees and separately unclaimed processes | Public nodes lack durable Run/Task identity; OS sleeping does not mean waiting for human input |
| Work event history | `WorkActivitySnapshot`; `lf activity` | Bounded recorded Work facts, Run/PR/Steer events | Current live Run inventory or native output |
| Flow execution | `FlowPosition`, `TaskWorkerClaim` | Current Task controller position and exact worker ownership | All generic/interactive Runs attributed to the Task |
| Flow history (LOO-293) | `TaskFlowEvent`, `TaskFlowStage`, `TaskWatchSnapshot`; `lf task watch` | Retained selected plans, attempts, return edges and Run-stage bindings | A bound/active stage is not proof its Run is alive |
| Output (LOO-293) | `OutputRecord`, `TaskOutputPage`; `lf task output` | Passive normalized journal/native records, revisions, source ordering, gaps and continuations | Run liveness, complete capture or cross-Run causal ordering |
| Pane | `PaneState`, `PaneContent`, `MultiplexerStore` | Window-local layout, selection, close/Undo, zoom and shell launch presentation | Work lifecycle or provider ownership |
| Terminal | `GhosttySurfacePool`, `TerminalIdentity`, client receipts | Native surface and actual client attachment | Task identity from cwd alone |

Sources: `swift/Loopflow/Models/{WaveWorkMap,SessionRecord,ActivitySnapshot,
WorkActivity,MultiplexerLayout}.swift`, `swift/Loopflow/Services/RegistryQuery.swift`,
`rust/loopflow/src/{durable,run_record}.rs`, `rust/loopflow/src/lf/commands/{runs,top}.rs`,
`rust/loopflow/src/ops/human_session.rs`; Watch source paths below are in LOO-293.

### Identity is a graph behind a simple outline

The navigation hierarchy is the main human presentation, not a demand that every
backend entity be a child in a universal tree. A Run belongs to its declared
Work subject, can have a parent Run, and can optionally be bound to a Flow stage.
A Task may have multiple concurrent generic Runs outside its declared Flow.
A Session must point to its Run, and may also represent an Ask/Flow boundary.
A shell can host successive or multiple Session associations. Monitor belongs
to a Task; it does not require a Session or a provider to exist.

Do not insert Run/Invocation/Exec levels into the primary outline. They are
Monitor details. Non-Task Sessions remain directly under their actual subject;
unattributed Sessions remain reachable without fabricating a Task.

Current `WorkspaceNodeKey` uses a repository and `WorkReference` containing
planning Project/Task IDs, while Session joins use `runtime.workId`. The explicit
join is correct; reusing the same Swift type for both ID domains invites mistakes.
Keep the stable planning key and optional durable Work reference visibly distinct
in the UI projection. Do not re-key a row when runtime Work first appears, and
do not invent a new persistent navigation identity.

All reads/actions must retain their selected Home authority. The earlier Home
mismatch in `scratch/session-discovery.md` is a real counterexample to treating
repository path as complete scope. A shared reading/cache key includes its Home
context; multi-Home aggregation must carry routing, not merge payloads and send
actions to whichever Home is ambient. Local failure says nothing about remote
liveness. This does not require a new cross-Home UI in the first increment.

### Sessions: required Run identity changes creation, not just serialization

Observed in `ops/human_session.rs`:

- Interactive `SessionRecord.id` is already `manifest.run_id`.
- Ask has an independent `ask_…` ID, a parent Run and optional `session_run_id`.
  The parent Run is the caller, not the Run conducting the human conversation.
- Flow ID encodes Task/invocation/flow/node/iteration. Its current position has
  optional `session_run_id`; a waiting boundary can be listed before launch.
- `spawn_session_run` launches a child, waits for its published Run binding and
  resumability, then writes the Run link into the Ask record/Flow position.
- `open_boundary` can clear a failed/unresumable Run link and launch a replacement
  while retaining the same Ask/Flow boundary ID.

Therefore adding an optional DTO field or setting run_id = session.id for all
kinds does not meet the human requirement. Neither does assigning Ask's parent
Run or a previous autonomous stage Run. Hiding waiting boundaries until a
provider starts would lose required human work.

Recommended target:

1. Create a resolvable Run identity and minimal provenance before publishing its
   Session. Provider launch is a later operation on that identity. Identity
   allocation must not launch a provider or manufacture a completed attempt.
2. Expose required `Session.run_id`. Prefer `Session.id == run_id` where it removes
   a lookup; keep title and readable Work path separate. Keep the field explicit
   even if values coincide, so callers need no convention-based parsing.
3. Preserve Ask/Flow decision tokens as boundary references. They validate the
   exact waiting caller or playhead; they need not serve as navigation IDs.
4. Resume/transfer preserves Run identity. A genuinely new replacement Run gets
   a new identity and the boundary explicitly rebinds; stale decisions must still
   fail. Do not silently retarget a retained terminal keyed by the old identity.
5. Run availability and live execution remain distinct. A prelaunch Run has a
   valid identity with no current live execution; Run history must describe this
   honestly rather than count it as a successful provider launch.

This requires coordinated changes at Session preparation, Run capture, child
launch binding, recovery, persisted Ask/Flow data and action lookup. It is not
just a Swift DTO edit. Current `CaptureHandle::begin_*` allocates a fresh Run ID;
the launch path must consume the prepared identity instead of minting a second.
Do not repurpose inherited `LF_RUN_ID` as a generic override: current execution
context uses it to identify ancestry. Existing stored boundaries need an explicit
migration/lazy preparation at a mutation boundary, not writes from list reads.
Every published Session must satisfy the same contract after migration. Exact
storage mechanics remain an implementation design decision.

## Tensions and evidence

### The active-Run join is missing

`ActivityNode` exposes Exec IDs and provider PIDs, but no durable Run or Work.
`RunEventRow.run_id` feeds `ExecRecord.trace_id`; journal's corresponding type is
`TraceId`, not durable `RunId`. Name equality is not a join. `RunManifest` retains
Run subjects/parent but no general active Exec binding.

Two narrower joins already exist: native provider-client receipts in a Run's
directory, and `TaskWorkerClaim.worker_run_id` paired with an exact owner
Exec/PID/start time. The latter covers a declared Task worker, not every generic
Run. Headless launch capture has Run identity in memory/environment but the
reviewed generic path supplies no equivalent public live Run observation.

Recommendation: establish a shared Rust live-Run projection using exact ownership
and capture bindings. Extend the common launch/capture ownership evidence where
needed, rather than infer from cwd, skill name, process ancestry alone or a
Task's current Flow. A live long-lived Exec cannot keep every historical Run it
once hosted alive; binding includes the currently executing interval. Reuse the
existing process observer and native receipts, including moved/resumed clients.

`provider_client_is_live` currently spawns `ps` per client and collapses command
failure to false. `top` samples a process table once and has different matching
rules. Consolidating the observation machinery can remove duplicate work and
preserve unknown evidence. These are source findings, not measured bottlenecks.

`ActivityState` currently derives working/waiting/stalled from kernel state.
Do not present that as semantic agent progress or human attention. No normalized
output-rate field exists in the reviewed Activity DTO (including canonical main).
Completed `RunUsage` totals are not instantaneous throughput. Richer rates later
need an explicit unit/window/source and available/unknown state.

### Watch is coherent for Flow inspection, too broad for initial Monitor

LOO-293 `ops/task_watch.rs` deliberately separates recorded progress from liveness.
Its SQLite reader gets position and all Task events in one transaction, preserving
real plan/attempt facts. `TaskFlowStage` identifies invocation, step index and
iteration; names alone would lose repeated stages. Keep these distinctions.

However `read_task_watch` reduces all retained Flow history and enumerates all
Home Run manifests. `task_run_manifests` matches Task Work ID or issue selector,
includes old and auxiliary Runs, and reports discovery gaps. This avoids the
ordinary `lf runs` fifty-record/seven-day presentation cap, but discovery cost
still grows with all retained Runs. It also differs from Session attribution,
which uses the shared binding resolver. Unify subject resolution in Rust rather
than maintain separate selector policies.

`TaskWatchStore` combines snapshot/Flow selection with output source retention,
filters, revision merging and two continuation cursors. That is reasonable for
its existing full Watch screen, but the active-Run list must not depend on loading
it. `WorkspaceNavigation.watches` retains one store per visited Task without an
overall bound. `TaskWatchView` mounts a separate Watch route and its own nested
split views. Initial load reads the plan then output. Refresh is manual; it is
not a proven live polling implementation.

### Output is reusable, but its bounds are partial

LOO-293 normalizes passive journal/Claude/Codex/OpenCode history into existing
`ConversationEvent` values with source IDs/revisions and explicit gaps. This is
valuable reusable work. Output reading does not open/move/resume the provider.

Current source pages cap records at 128 and scanned bytes at 8 MiB; the Task
reader visits up to eight sources. Every request still scans all manifests and
Task events; tail initialization seeds all discovered sources. The cursor holds
per-source state and rejects encodings over 4 MiB. Swift retains at most 4,096
records and 16 MiB accounted payload per Task, not a global memory/RSS bound or
an incoming decode bound. These limits do not make total discovery/initialization
bounded. Do not enable frequent polling merely because payload retention passes.

Integration counterexample to test: `task_output::output_source` chooses native
history only for original `manifest.surface == "tui"`. This branch explicitly
supports a headless Run later resumed interactively (documented in
`.lf/directions/desktop.md`). The selection rule would continue choosing its
journal. Source inspection establishes a contract mismatch to resolve; no fresh
configured trial was performed to measure missing output. Preserve source/epoch
provenance rather than overwrite original launch mode or blindly concatenate
journal/native copies.

## Recommendations

### API direction: domain reads with independent costs

Prefer extending the **Run read family** for live Task Runs over adding a generic
`TaskMonitorSnapshot` that duplicates planning, Sessions, Flow and output.
Proposed command spelling: `lf runs --task <selector> --active --json`; this is
not implemented or a compatibility promise. Its observation envelope carries
Home/scope, observation time, completeness/gaps and stable Run identities with
resolved typed Work, label/provider and exact current execution evidence.

Keep durable metadata/outcome and live observation separate in that envelope;
do not overload `outcome == nil` or `TaskWatchAttemptState`. Reuse current Run
fields where they mean the same thing, but do not require reducing usage or
transcript history merely to list active Runs. An active query must discover
live bindings first and resolve their manifests, not filter a capped history
response. Partial evidence cannot produce a healthy zero. Avoid per-Task
subprocess polling when several Task monitors are visible: one shared scoped
observation can serve their filters.

Expose Sessions with required Run links through the existing Session family.
Keep their legal actions there. Monitor can link an exact Run to an available
Session using explicit IDs. Not every Run has a Session; passive Monitor never
creates one implicitly. Current TaskFlowStage/plan inspection can remain a
separate read; the existing `task watch` command may serve it while naming is
reconciled. Renaming alone has no architectural value.

Retain `task output` as a paged aggregate over reusable per-Run output readers.
An individual Run detail should use the same reader, not introduce another
provider adapter. History and output load only on demand. First Monitor needs
neither a diagram nor transcript history to render its live list.

Compared alternatives:

| Approach | Verdict |
|---|---|
| Make Watch the Task's universal model | Reject: Flow/history-centric, costly first read, missing liveness; duplicates other authorities if expanded indiscriminately |
| Join roadmap, ps, Runs and Watch in Swift | Reject: clients would own identity/liveness inference and repeat it; current wire data cannot establish the join |
| Extend only ps with Task labels | Insufficient: process trees and durable Runs are different cardinalities; labels do not supply Run identity |
| Shared Rust Run observations using the same process evidence as ps | Preferred: one exact join, independent live cost, CLI/Mac parity; requires real launch binding work |
| One giant workspace response | Defer: atomic planning/runtime freshness is not currently available and would couple slow history to fast navigation |

### Pane ownership and outline projection

Add Task-bound Monitor content to existing `PaneContent`; keep pane instance ID
separate from its Task subject. Existing multiplexer owns split/focus/zoom/Undo;
window registry owns surfaces and checkout placement. Readings belong to the
shared model; a pane owns local Run selection/filter/scroll. Two visible panes
for one Task share a reading, not necessarily the same filter. Hiding Monitor
can stop demand-driven reads without destroying terminals or their drafts.

The current multiplexer replaces a focused non-shell pane when loading a Session.
Adding a Monitor case requires deliberately retaining/focusing existing content
or splitting it; copying that replacement rule could erase the retained Monitor
pane. Generalize content operations only as needed. Do not add a second split
store or mount a terminal into two panes.

Keep `WorkspaceProjection` as a derived outline over planning + Sessions. Build
ID indexes once per accepted snapshot; compression/search transform visible
rows. Today it repeatedly filters Sessions for every subject. This is a potential
cost to measure, not evidence that an extra cache or persistent tree is needed.
Process/output refreshes should not recompute unrelated hierarchy structure.
Unavailable ancestry remains explicit; singleton compression requires complete
structural evidence, not whichever processes happen to be alive this poll.

### Two user-experience performance contracts

1. **hierarchy_interaction_ms:** accepted expand/compress/filter input to correct
   visible, usable rows. Exercise small/large fixed hierarchies, Sessions-only,
   duplicate titles and scrolling during refresh. Record frame hitches too.
   This path must not read Flow history, transcripts or per-row process state.
2. **task_workspace_ready_ms:** Task/Session selection to the correct visible,
   usable Monitor or retained terminal. Separate warm/cold and Monitor/terminal
   scenarios. Terminal proof includes actual focus/input to the same owned PTY;
   Monitor proof includes exact Runs or a confirmed empty/error state. Add a
   mixed Monitor/terminal resize-and-return scenario. Do not time provider launch
   as if it were a retained-pane switch.

Use correlated read/decode/projection/layout/presentation phases for diagnosis,
per-attempt machine-readable outcomes, failures/timeouts and identical populations
for before/after comparisons. An AX control observation is not pixel paint; a
model assignment or onAppear is not usable readiness. Existing SessionsLatencyMetrics
logs store-load/pane callbacks and lacks this complete endpoint/hitch proof.

Publish baseline and budgets before optimizing. Keep one first-pass optimization
Task per experience after the measurement contract is implemented. Bound live
reads/output discovery as part of correctness; measure before adding indexes,
extra caches or broader streaming infrastructure. No numerical performance claim
or latency baseline was collected in this review.

### Disposition of LOO-293

Preserve its implementation and proof. Reuse Flow history receipts, passive output
normalization, revision/gap handling and meaningful tests. Adapt Run discovery,
Session links, source selection, reading lifetimes and bounds. Replace the
separate Watch navigation/split composition when integrating into the multiplexer.
Do not port the entire view as the initial Monitor or close the Task/worktree
before its valuable work and remaining obligations have a reviewed destination.

The sibling was initially clean at `13fc09ea0` (it advanced since the earlier inspection
at `7b9d5d87d`). This review did not make that commit or claim its tests. Current
LOO-291 is dirty at `9b3264efd`; canonical main read for Activity comparison is
`7160f3637`. No merge, checkpoint, PM write or Task disposition was performed.
The final check found new untracked `scratch/review-window-proof.py`,
`scratch/review-window-proof.swift` and `scratch/watch-window-evidence/review/`
in LOO-293. Another contribution is in progress; these files were left untouched
and are not included in this review's proof claims.

## Verification, quality and open questions

Read Rust producers/ownership/storage, Swift mirrors/readers/presentation, DTO
fixture assertions, existing focused test bodies and the sibling retention proof.
Existing tests cover old/auxiliary Run discovery (51 Runs dated 2020), output
revisions and independent live/history continuations, cancellation/reset errors,
retention bounds and actual stage selection. They do not prove a live Task Run
API, required Session Run identity, combined multiplexer panes, bounded total
polling cost or the new performance endpoints. Prior passing receipts remain
prior evidence; no unit test or configured app/provider action was rerun for this
read-only review. Source hashes below identify reviewed contracts, not test passes.

Implementation acceptance must falsify the tempting shortcuts:

- Upcoming Task without runtime; single-child compression with incomplete data;
  duplicate Session titles; same selection through full/compact/flat presentations.
- Waiting Ask/Flow already has a resolvable Run; lookup agrees by Session and Run;
  initial launch consumes that identity; resume/move preserves it; replacement
  and Iterate preserve exact boundary targeting and reject stale decisions.
- Generic/Flow/interactive Runs of one Task concurrently; two Tasks share cwd;
  one Exec hosts sequential Runs; an old live Run survives history limits;
  stale/reused PID, unknown observation and remote Home do not become false live/zero.
- Headless Run resumed interactively; unavailable native output; source reset;
  no provider launch/transfer caused by observing Monitor.
- Same terminal/draft/input survives Task navigation, Monitor split/zoom and
  closing Monitor; repeated views share reads; failed refresh retains labeled
  last-good data; total retained state remains bounded.

Open implementation questions, not missing human approval: how best to prepare
Run identity before launch using existing capture/storage; how to persist a
minimal exact active Run↔Exec interval for generic launches; how to migrate
existing Ask/Flow boundaries without list-read side effects; which source/epoch
contract covers interactive resumption; measured budgets and baseline population.
These questions must be settled before claiming the new API implemented. The
human's required Session.run_id invariant and the single-outline/multiplexer UX
are settled direction.

## Reviewed contract fingerprints

- LOO-291 `rust/loopflow/src/ops/human_session.rs`: `33bd05a8d693f0d16d9add3cea163b99808d0835cf792d1fb641620b013afcfb`
- LOO-291 `rust/loopflow/src/run_record.rs`: `aaeb6e12984dce732fa7ccca83e9704b9b180efcdf1b38a47ad98d82c1761e68`
- LOO-291 `rust/loopflow/src/lf/commands/top.rs`: `990ad08f70094542c8fb67e8cb951541ed15eacb78978170699964ab83542ded`
- LOO-291 `swift/Loopflow/Models/SessionRecord.swift`: `92bab46706cb16915959d297b268088a6550b536eca496f706b4e6275716b5c0`
- LOO-291 `swift/Loopflow/Models/MultiplexerLayout.swift`: `fdc44a9970d977865d964b998893920d5e76cd7e6e8854ee848000b4b462beb5`
- LOO-291 `swift/LoopflowMac/WorkspaceProjection.swift`: `6aa6e425e009ab94855b76aa764d17477c924d0df33737bfa8e826cf9cfc4598`
- LOO-293 `rust/loopflow/src/ops/task_watch.rs`: `51403bfe4b4d1b49232b984e64428f7d260508790b8c7381e9b9d405b10ee226`
- LOO-293 `rust/loopflow/src/ops/task_output.rs`: `e7acd31d6341686c339a2331f91234072e1d0bec70370d534db7d7974262635d`
- LOO-293 `rust/loopflow/src/run_record.rs`: `807062bbe9c5b5d740d80f18a036be533c1c213466916b02293c53086aa338d0`
- LOO-293 `swift/LoopflowMac/TaskWatchStore.swift`: `6f47916805579b459615e9a1066457833ecfcbdf6c1447307f6d963642d88313`
- LOO-293 `swift/LoopflowMac/Views/TaskWatchView.swift`: `7dd12af0c1748ee8163da6e906558dc25c5e90b25593b59ab46f7afaf7ea4085`

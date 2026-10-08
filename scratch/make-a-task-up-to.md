# One Task, up to one PR

LOO-418 · completion trigger implemented locally; shared planning integration remains · reconciled 2026-10-08

## Decision and intended experience

Jack decided on October 7 that a Task delivers zero or one PR, never a serial
chain. Follow-through is a follow-up Task: file it after landing, then complete
the landed Task. Dependent implementation starts in another stacked Task before
its parent merges. A recommendation can finish without a PR.

Jack Heart confirmed the completion rule during design review on October 7:
`PR merges → file follow-ups or record “none needed” → Task completes`.
On October 8, Jack Heart revised the earlier end/completion equivalence:
“end -> complete should be like a trigger though then, not state.” Reaching
Workflow end requests completion; Task completion, Workflow position and Process
liveness are independent facts. `task complete` is a distinct operation again.
Linear completion changes Task status without moving the Workflow or inventing
Process exits. Jack requested updating this design and continuing implementation.
This supersedes October 7's alias decision and the corresponding implementation
at `5876a8692`. The full change remains on PR #1499; landing is unauthorized.

The experience: a Task has one outcome, an optional PR link, and one Workflow
position, independent of its completion status. After normal delivery, its page says either **Done · Follow-up LOO-…**, or
**Merged · Follow-through pending**, with the reason and next action. It never
says “PR 2,” retains an unexplained open state, or waits for production evidence
that belongs to another Task. Research reaches `end` without a phantom PR slot.

## Alignment with local planning (2026-10-08)

Jack Heart requested avoiding duplicate data types and making repair explicit in
`--wait-and-fix`. The landing flag replaces `--wait`; ship and all callers use
that spelling. Bare landing still returns after requesting merge.

LOO-406's earlier published local lifecycle is superseded by its ongoing
common-ownership revision. Its October 8
brief and checkout use the existing Task, TaskPlan and TaskSnapshot, with optional
Linear mapping and checkout. TaskSnapshot is the assembled read, not another
stored Task or planning authority. Preserve one model and one read path when the
branches integrate: local identity and optional placement from LOO-406, one
optional PR and follow-through from LOO-418. Do not introduce local/Linear or
delivery-specific Task copies. Shared fields should come from their owning record;
computed status, work and actions belong in the read projection.

Integration remains: reconcile overlapping Task/TaskSnapshot and Rust/Swift wire
changes, make branch/base placement absent until checkout, and route follow-up
creation, links and completion through LOO-406's shared local planning APIs.
The current follow-through and completion implementations still call Linear
directly; merging DTOs alone will not make either path work without Linear. Verify unplaced,
PR-less and merged-with-follow-up Tasks through both CLI and Desktop, including
local filing/completion without provider calls. No second filing or completion
implementation should survive. LOO-406 was rechecked read-only on October 8 at
`cbdb9a43b`, with uncommitted design updates; no live Task/PR state was refreshed.
Saved Tasks now share offline checkout preparation. Completion and reopening save
locally before provider I/O, while the coherent common-writer cut remains
unavailable. It deletes `complete_planning_task`,
`task_pm::complete_task`, `reconcile_pm_writeback`, the completion arm of
`PmTaskUpdate`, and direct writeback updates. These are integration deletion
targets here; do not ship the current provider-first completion path beside them.
No changes were made in that checkout. The ongoing plan is evidence of unfinished
work, not a stable integration commit or new authorization.

Two integration conflicts are now explicit. LOO-406's `ops/task.rs::reach_end`
still couples completion to Workflow movement, including the old planning-conflict
and dirty-checkout refusals. Integration must retain LOO-418's independent status,
durable end request and retained-artifact behavior while adopting the local save
and optional delivery receipt. Replacing this branch's operation wholesale would
restore the behavior Jack superseded.

LOO-406 records an enabled failing regression,
`task_completion_preserves_linear_reopening_during_delivery`: a provider reopening
between ownership read and completion mutation is overwritten, and matching
readback falsely settles the receipt. Its test and unconditional writer were
inspected; the recorded failure was not rerun here. This branch also checks its
request before an unconditional provider update, so its passing observation tests
do not prove preservation of a reopening during that write interval. Carry the
regression into composed acceptance. Extra reads and matching readback do not
establish a conditional-write guarantee. Provider-guarantee investigation remains
with LOO-406; neither automatic propagation nor concurrent-change preservation
has been relaxed. If no sufficient guarantee exists, that exact product tradeoff
remains unresolved. Common local ownership can progress independently.

## Demo

In an isolated fixture repository, start a child Task while its parent's PR is
open. The child opens with its own brief and the selected design excerpt, with
no copied “This slice” pointing at the parent. Publish the child's PR before the
parent lands. Sync a parent edit, then its squash merge: the child keeps its code,
notes, Session and PR, and targets main.

Land the parent through its `ship` edge. After authoritative merge, the
follow-through step files “Verify the released command on the installed version,”
linked to the parent and carrying a due date and evidence condition. The parent's
CLI status and Desktop page show Done and that link. Retrying after a simulated
lost Linear response returns the same follow-up. The follow-up owns the later
check; advancing the clock alone completes neither it nor its acceptance.

Finally run a research Task to accepted findings and `end`: no PR is created or
shown. Throughout, launching from a sibling checkout shows the actual Flow under
the target Task, without the Started warning.

The four lifecycle slices, landing corrections and independent completion trigger
are implemented locally. The shared local planning integration is not built. A Task/Flow/waited-land fixture now repairs CI and reaches follow-through;
repeated landing preserves finishing notes for pending and completed Tasks.
Shared local planning integration, complete lifecycle proof and Jack's demo review remain.

## Inventory and findings

Initial inventory: `626789dcd0382c6754ba3b7c61ea2448b57658ce`. Reconciled
October 7 after integrating `35e759aaf`: #1492 supplies the FlowProcess API and
`flow list --processes`; #1495 simplifies completion readiness and keeps
terminal Task actions inert. The implementation preserves those changes. This inventory records the
pre-change mechanisms; ship now runs `gate → land --wait-and-fix → follow-through`.

| Existing mechanism | Finding and treatment |
| --- | --- |
| `ops/task.rs`: `prepare_new_task`, `create_prepared_task`, `restore_task_checkout`; `work/task/mod.rs`: `TaskPr` | Every checkout gets a working PR record. That record owns branch/base and is required even to restore a research checkout. Move placement onto Task before making its PR optional. Hiding a slot only in Swift cannot fix this. |
| `ops/task.rs`: `pr_next`, `rotate_task_pr`, `next_pr_slug`, `deterministic_next_branch`, `roll_back_failed_rotation` | Serial rotation waits for the current merge, carries commits and dirty edits, then changes the same Task's branch. Delete rotation, preserving protection of unpublished files and historical receipts. |
| `AfterMerge::{ContinueTask,CompleteTask}`, `MergeRequest`, `PrLanding`, `land/arm/submit` flags | Current source already defaults bare landing to completion; `-c` is redundant. `--next` preserves the alternate chain. Supplied older operating text describes a different default. Remove both flags and the disposition enum, not a second compatibility mode. |
| `cead4c952` / #1488, LOO-408 | Merged delivery now completes by default without stale Process/Session gates. `TaskFollowUp { outcome, evidence, check_at }` and `task follow-up --clear` deliberately keep the original Task open. Replace this current writer and projection; preserve its existing unresolved obligations during conversion. Do not restore execution-completion gates. |
| `ops/task.rs`: `reach_end`, `task_completion_gate`; `store/sqlite/children.rs`: `complete_task` | Workflow owns done. A no-Flow edge may retain dirty files, but an unpublished PR with commits still blocks completion; a placeholder only disappears if its branch is empty. Optional PR requires removing that dependency, not another exception. |
| LOO-385, read with `lf task status LOO-385 --json` | Planning read reports unstarted/no local execution, observed_at 1791421119. It asks for Workflow-derived state and no PR slot. Much state derivation exists; the slot/placement coupling remains. This design incorporates that behavior. No issue closure or reassignment was performed; reconcile overlap before parallel implementation. |
| `feature.yaml`, `code.yaml`, `research.yaml`, `ship.yaml` | Feature already stops for design and demo; research has an empty edge to end. Ship is `gate → cmd: pr land -c`. Land returns after requesting merge, so successful command exit cannot mean the Task reached end. |
| `task_stack`, `record_stack_sync`, `ops/sync.rs` | Existing stack code records parent PR/fork, integrates a live parent, and handles squash landing. `sync_tests` contains child-edit, scratch, squash and conflict cases. Preserve this mechanism. Parent merge does not require replacement child identity. |
| `finish_task_checkout` | The child's first commit deletes inherited scratch; subsequent sync preserves the child's scratch. This is intentional isolation, and explains the manual design copying in LOO-394. Add explicit design handoff; do not restore wholesale inherited scratch. |
| `pm_create_task_idempotent` | Existing filing stores a content marker and reconciles uncertain creation, but searches the currently selected Project. A retry after rotation can miss the original issue. Follow-through needs a persisted destination and issue identity before external mutation. |
| `task_operate.md`, `wave_operate.md`, `ops/cron.rs` | Operators leave unstarted backlog alone. The minute reconcile command observes delivery and does not launch Flows. A due date is not an installed timer or permission to start work. |
| #1439 / `9c7b6828a`, current CLI command tree | Flow resume and Session ready/complete are removed. A stopped Flow remains history; no replacement resume/acknowledgement controller is required here. |
| `task_session.md` with included `task_operate.md`; installed `task/session` | No explicit completion call; guidance stops when the Task lands. Update it to finish follow-through and verify completion. The restored Task command does not restore Session completion controls. |

### The launch defect has a shared cause

The base admitted the Process before `--task` placement and launched the Flow
child from the caller's cwd. Started and Flow membership use that recorded
location. The milestone now resolves existing `--task`/`--wt` placement before
admission, observes resolution failures, and gives Task Flow children their
checkout explicitly. Physical cwd resolves lexical aliases. Flow inventory
uses the shared checkout/Session membership predicates; Flow registration and
Started commit together before any step launches. Historical Process rows are
unchanged.

A disposable SQLite probe executed the actual released Started trigger with one
Task and one recorded Flow. Caller cwd reproduced “Started requires recorded Task
work”; changing only that row's cwd to the Task checkout allowed Started. This
proves the predicate failure and, with the dispatch ordering, a concrete defective
path. Public CLI coverage now exercises sibling, direct `--task`, plain checkout,
symlink and `--wt` launches. Neither fixture replays every October 7 launch.

### Provider support

Linear's official generated SDK defines `IssueCreateInput.id` (UUID v4),
`dueDate` (`TimelessDate`) and `IssueRelationCreateInput`, including related issues.
The existing Linear client now creates issues with persisted UUID v4 identities,
dates and related-issue links; PM projects dates and follow-up sources. Simulated
provider tests cover lost responses, issue edits/moves and chapter rotation.
Live provider writes remain unproven. Confirmation takes the observed due date,
including removal, while the retry intent keeps its original payload.

Sources inspected October 7: [Linear SDK schema](https://github.com/linear/linear/blob/master/packages/sdk/src/_generated_documents.ts),
[issue relations](https://linear.app/docs/issue-relations),
[due dates](https://linear.app/docs/due-dates).

## Accepted decisions and implemented mechanisms

### 1. Follow-through belongs in the landing Flow, after verified merge

**Accepted by Jack Heart, 2026-10-07:** every merged Task receives an automatic
follow-through check, including a manual GitHub merge:
`PR merges → file follow-ups or record “none needed” → Task completes`.
For LOO-407's case, the source Task completes after filing the installed-release
check; the new Task owns obtaining that evidence. A refactor with no remaining
obligation completes after recording none, without another review click.

If no finishing Flow or operator runs, even a routine merged Task stays visibly
pending. Merge alone cannot establish that no follow-up is needed. Jack accepted
the normal landing Flow plus recovery by the next Task/Wave operation, without
a new watcher. An open conversation does not schedule its next turn; unattended
recovery depends on an installed operator schedule. The polling limits and filing
interface below are implemented defaults, not separately approved decisions.

`lf land` and `lf arm` remain prepare/request/return operations. Land supports
`--wait-and-fix` for callers that own a blocking Flow. It uses the existing finite
landing observation operation and shared CI repair admission, releasing its
lock between checks. Poll every 15 seconds for at most 30 minutes;
timeout returns held (exit 3), leaving merge intent and evidence intact.
Interruption uses the existing global handler (exit 130), which stops a Flow
without retry and retains merge intent. A second Tokio signal handler conflicted
with that owner in the public CLI test and was removed.
These are initial operational defaults, not measured performance claims.
Jack Heart confirmed on October 8: waited landing must repair its own PR with
Desktop closed. It uses the existing incident reservation and repair worker;
the optional repository watcher shares that path. Bare land still returns:
changing that default was suggested, not accepted. Repeated land on a merged
Task PR succeeds before preparation, reports completion or remaining
follow-through, and preserves edits. Ordinary reconciliation starts no repair.

The historical switch to finite landing (#1382) provided continuation after
caller exit; #1384 moved automatic repairs to the optional watcher. #1287 fixed
repair sandbox permissions and retained locks while canceled waits still had
effects running. No inspected evidence identifies a particular land crash as
the reason for the switch. Task run currently retries an entire failed Flow up
to three times while its carrier survives. Jack requested retry from the failed
step on October 8; Infrastructure [LOO-435](https://linear.app/loopflow/issue/LOO-435)
owns that separate design. The current correction preserves held exit 3 through
the Flow instead of converting it to failure and replaying gate.

Change `ship` to `gate → cmd: land --wait-and-fix → follow-through`. The `follow-through`
skill files or links the remaining Tasks, records the disposition, then calls
`lf task complete ISSUE`. The command itself runs no skill and files no Tasks.
The skill completes the Task without moving its Workflow. The enclosing driver
then records its actual arrival at end; its completion trigger is idempotent.
Completion failure after a successful edge preserves arrival and retries only
completion, never the edge or gate. No new Workflow review
node is needed: routine filing is autonomous,
while an unresolved scope decision is a reported blocker in the existing Task
conversation. A bare land, reviewer merge, or out-of-band merge records
**Merged · Follow-through pending**. Reconciliation records facts and can finish
an already recorded disposition; it never manufactures an agent verdict.

The existing Task/Wave operator may launch a fresh `finish-delivery` Flow for a
merged Task whose original Flow is gone. That Flow runs only `follow-through`
which ends with the same completion command. It inspects every associated live Flow
first. It does not rerun gate, re-arm the PR, resume an old Flow, or need the
Workflow template to still exist. A live ship Flow owns its own finish.

Update `task/operate`, and therefore its included instructions in `task/session`,
to continue past merge until follow-through and completion are recorded, or a
concrete blocker remains. If the disposition is already durable, call
`lf task complete ISSUE` and reread status; otherwise run the remaining
follow-through. A successful agent turn or Flow exit alone never proves done.

Give the follow-through skill the accepted Task brief, recorded acceptance
limits, merged PR/head, delivery evidence and any existing linked follow-ups.
Landing skills identify likely follow-through before scratch cleanup and include
the exact unverified outcome in durable PR copy. The post-merge skill reads that
copy and the brief; deleted scratch is never a required input.

Keep the checkout available while follow-through is pending. Merge observation
must not complete the Task or retire the checkout before the next Flow step can
run. After Task completion, existing cleanup may retain a checkout
for its live Flow or Session without keeping the Task unfinished. Prove this
ordering with reconciliation running between merge and the follow-through step.

File a follow-up for a concrete accepted obligation whose evidence becomes
available later: installation, real usage, production behavior, an agreed dated
measurement. Its brief contains the source Task/PR, the observation to make,
environment/version prerequisite, expected evidence, due date if meaningful, and
what to do on failure. Do not file “monitor this” after every refactor. Newly
imagined enhancements are suggestions, not accepted work or automatic filing.
No accepted remaining obligation means record `none` with a short reason and
complete. Ambiguous accepted scope remains visibly pending for Jack's decision.

`lf task follow-up ISSUE` now files or links actual Tasks, or records that none
is needed. It replaces `--outcome/--evidence/--check-at/--clear`. Supported forms
are `--title … --notes … --due YYYY-MM-DD`,
`--existing ISSUE`, and `--none REASON`; allow repeated filings before recording
the disposition complete with `--finish REASON`. `--none` resolves a disposition
with no filings; `--finish` verifies all recorded intents and their links before
committing the filed disposition. Neither waits for the children to finish.
The skill, not the CLI, judges the brief. Filing may
span owning Waves; an explicit destination wins, otherwise use the source Wave's
current Project at the time filing begins.

Use existing append-only Task events for the disposition and request receipts:
pending, filing intent(s), then none(reason) or filed(issue IDs, reason). This is
evidence for completion, not another Task state machine. Serialize intent creation
under the existing Task control boundary; each intended child gets a stable
request key, persisted UUID v4, Project ID and exact payload before sending.
On uncertainty, query that UUID and pinned destination; do not recalculate the
Project or use a changed title as a new request. Freshly confirm issue and link
before recording filed. Repeated calls reuse receipts, including after a crash
between provider success and local writeback. A second deliberate obligation
gets a different request key. Never hold a local lock across an agent turn.

Linear owns the follow-up's authored plan and date. A native `related` relation
gives navigation without making the child block its completed parent. The child
brief says “Follow-up to …”; the parent's disposition records the direction and
exact issue IDs. Desktop shows those links from the shared projection, rather
than guessing from comments. A native relation alone does not encode direction.

### 2. Stack Tasks and hand off only the intended design

Keep `lf checkout CHILD --stack-on PARENT`, `lf task run`, and `lf sync` as the
placement and integration paths. Initial scope retains the existing published
parent PR requirement; a draft suffices. Starting or publishing the child never
waits for the parent's merge. Landing it still waits for that dependency.

Extend `lf checkout` with optional `--design PATH` for an explicit handoff. It
reads a file from the caller's checkout and writes the child's
`scratch/<child-slug>.md` before any agent starts, without overwriting newer
child work. Store source Task, source commit (and content hash for uncommitted
input), and selected content in the handoff receipt. Retrying the same input
does not reset the child. Different content at an occupied destination is a
reported handoff conflict with both contents retained.

The handing-off operator prepares a self-contained child-specific design from
the common plan and the child's brief. `ship-decomposed` and capture guidance
file one Task per independently deliverable PR and prepare those excerpts before
launching children. Do not copy an entire eight-step plan and mechanically toggle
one “current” heading in eight places. Mechanical checkout transfers content;
it does not pretend to interpret Markdown or approve the child's scope.

Code follows parent changes through existing sync. The child's design becomes
its own working artifact and is not overwritten by later parent scratch. New
scope decisions travel through explicit Task direction and deliberate design
updates. A parent merge changes the child's base to main through existing squash
handling; preserve the child's Task/PR/branch/Session identity and scratch.
Missing or abandoned parent evidence stays an explicit dependency problem,
never an implicit reparent to unrelated code.

### 3. End triggers completion; it does not define Task state

**Accepted by Jack Heart, 2026-10-08.** Task completion records the disposition of
the work. Workflow position records where its execution got to. Processes record
their own liveness and outcome. None is an alias for another. A completed Task can
have a running Process or an unfinished Workflow, and a Workflow can reach end
while its requested completion is pending.

`lf task complete ISSUE` calls the one completion operation without moving,
creating or restarting a Workflow. `lf task move ISSUE end`, an empty final edge,
and a successful final Flow record the real Workflow movement, then trigger that
operation. An already completed Task makes that trigger a harmless success. A
finishing skill's completion followed by driver arrival produces one completion
and the actual arrival history, never a fabricated earlier arrival.

Completion uses the shared Task/planning status owner, aligned with LOO-406's
local planning API; TaskSnapshot projects it alongside the existing Workflow and
Process reads. Do not add a second Task type, execution-derived completion flag,
or separate local/Linear completion implementation. Migrate the current
Workflow-derived state to independent durable completion while preserving
historical successful completions and their identities.

The end trigger must survive a crash after arrival but before completion. Record
its pending intent with the move, using the existing operation/event ownership,
and settle it idempotently. On refusal or write failure, retain end and show
**Completion pending** with the reason and `lf task complete ISSUE` as the retry.
The Task/Wave operator retries that operation, not `task run`, gate, land or the
already successful Flow. A delivery refusal still names its missing merge or
filing evidence. This is separate from LOO-435's retry of a failed Flow step.
Ordinary reads neither execute the trigger nor start workers; no new watcher or
per-Task scheduler is introduced.

For Loopflow-requested completion of a Task with a PR, preserve authoritative
merge and confirmed none/filed follow-through requirements. For a Task with no
PR, completion needs no landing ceremony. Committed findings, dirty files and
open Sessions do not manufacture a PR or stop valid completion. Cleanup is a
separate safe operation: retain live work and unpublished files.

When a fresh accepted Linear observation marks a Task complete, reflect that
Task status even if its Workflow is on an edge and a Process remains open.
Preserve the exact Workflow position, captured graph, Process outcomes, Sessions
and checkout. Remove the current planning conflict and its forced-end workaround.
Do not cancel an already running Flow or prevent its successful driver from
recording the real arrival merely because completion arrived first. Completion
alone authorizes no new work. Do not hide running work or pending delivery facts
just because ordinary Task navigation filters completed Tasks.

Linear's completion mark does not prove GitHub merged or follow-ups were filed.
Keep those facts and remaining obligations visible; do not fabricate a “none”
disposition, erase them, or reverse the authored completion to make Workflow and
Task status match. A newer reopening likewise changes Task status without moving
the Workflow or granting a second PR. Do not immediately re-complete a reopened
Task from its old end position or replay a superseded completion request; retries
must respect newer authored status changes. Stale/equal provider observations
retain the existing revision/conflict rules.

Task owns branch/base/stack placement and one optional PR. Preserve research file
browsing/restoration, publication intent/retry, historical PRs and `pr: null`.
Reopening a closed PR reuses the same PR; a different PR requires a different
Task. Delete `land -c` and serial `--next` as already implemented.
`--wait-and-fix` changes waiting and repair, not completion policy. Taskless
PR delivery still creates neither a Task nor follow-through obligations.

### 4. The owning Wave returns to a dated follow-up

Use Linear's due date for the day a check is wanted. Exact release prerequisites,
timezone and any time-of-day requirement stay in the brief; a date is not an
exact-time alarm. Extend the existing PM read/DTO to carry the optional date and
follow-up references. The owning Wave's next `wave/operate` pass lists due
follow-ups, even if unstarted, separately from unrelated backlog.

**Implemented authorization boundary, not separately approved by Jack:** filing
creates an obligation to return to, not permission to execute arbitrary future work. The pass surfaces a due Task
for selection unless its brief already authorizes a concrete unattended check.
An authorized check uses ordinary Task execution and the existing liveness
inspection. Missing credentials, installation, date or provider evidence leaves
the child visibly blocked/unknown. Time passing is never proof of success.

Use an already installed Wave schedule or its ongoing conversation. Do not add a
resident, a per-follow-up scheduler, or enable a schedule as a side effect of
filing. If no schedule is installed, report “next Wave pass; no automatic check
scheduled.” Exact unattended “tomorrow at 09:00” service is excluded. Implement
this conservative return path and show it in the complete lifecycle demo.

### 5. Record the actual execution location once

Resolve cheap existing `--task`/`--wt` placement before admitting the executing
Process; preserve early failure observation when resolution fails. `task run`
still records its placement/orchestration Process before doing work, then starts
the plain Flow child with `current_dir(task.worktree)`. Direct `--task … run …`
must receive the same treatment, without spawning a second driver. Admission
must not itself start a Task, create its checkout or contact planning merely to
inspect it. Record a resolved location so lexical aliases do not split membership.

Keep Process origin/causal ancestry separate from performed location. Do not
rewrite earlier usage, relabel historical Process cwd, infer ownership from argv,
or broaden the Started trigger to accept any inspection. Consolidate the Flow
summary's duplicated membership join onto the same shared association used by
Task inventory, including explicit Session bindings without following causal
parents. New Flow record and Started marking must either succeed together or
report an actionable storage failure before launching steps; do not hide it as
a warning while continuing invisible work.

## Migration and deletion cut

One Task draft, created through `scripts/new_migration.py`, tested from the
released frontier with this branch's finished schema. Do not edit released SQL
or add an altering draft on top of this Task's own unreleased draft.

Preserve existing Tasks, external issue IDs, checkout/PR identity, running Flows,
Session input and history. Move checkout placement out of working PR placeholders.
Unpublished placeholders become placement without a displayed PR. A published or
publishing current PR becomes the sole current PR, even when merged/abandoned.

Historical multi-PR Tasks cannot be made never to have existed. Retain their old
PR rows and references read-only, marked historical in the existing table, and
enforce at most one nonhistorical PR per Task with a partial unique index. Remove
all runtime traversal/rotation of the historical chain. Explicit history reads
may return those rows; normal Task DTOs do not. This is a bounded production-data
exception, not a parallel lifecycle: only migration can mark/create historical
rows, and no current operation appends a successor. Preserve landing and stack
foreign keys to old PR IDs. Completed historical Tasks remain complete. Seed independent completion from
released evidence during migration only; runtime reads must not keep deriving
completion from the Workflow node. Preserve open execution and pending provider
writebacks without inventing an exit or starting any operation.

For old `ContinueTask`/next-slug or unresolved `TaskFollowUp` decisions, retain
the original evidence and expose **scope needs conversion**. Do not infer that
an outcome is satisfied, silently close it, or create remote issues in a schema
migration. The ordinary follow-through path turns confirmed remaining scope into
new Tasks using the same filing receipts. A populated live chain whose newest
PR is active keeps that PR as its one current delivery; older deliveries become
history. No migration starts a Flow or transfers running work. Captured Flow
graphs and their invocation binaries remain historical execution inputs; do not
rewrite a running graph to remove an old flag. Newly compiled Flows and documented
commands must use the new command contract.

### Delete — do not maintain

Remaining integration cut: replace `ops/pm.rs::complete_planning_task`,
`task_pm::complete_task`, `ops/task.rs::reconcile_pm_writeback`, the completion
arm of `PmTaskUpdate`, and direct completion writeback updates with LOO-406's
common local planning writer. Route follow-up filing through that same planning
owner. Do not refactor or extend these predecessor paths while that boundary is
unfinished; no parallel local/Linear completion or filing owner may survive.

Implemented cuts: serial PR rotation and its CLI flags, placeholder PR creation,
PR-owned placement, keep-open follow-up writers, ordinal PR projections,
Workflow-derived completion, the Move/Complete alias, planning conflicts and
forced-end recovery. Independent completion, durable end requests and real
Process history replace their exclusive fixtures. Rust/Swift wire types, Desktop,
operators and docs use the surviving contract. Detailed deleted symbols and the
earlier alias design remain at `5c240c89a:scratch/make-a-task-up-to.md`.

Compression also removes the successor-suffix slug helper and action branch
matching the retired “Remaining work” summary. Historical events and PR rows
remain the explicit migration exception; writers leave retired disposition
columns untouched and the shared projection retains unresolved scope.

Retain exact-head landing generations, CI repair deduplication, Git mutation
locks, safe cleanup, Task history, shared status/monitor reads, symlink-safe file
access, and existing stacked sync. No new Flow primary selector, resume
controller, Session approval API, duplicate Task status store, or source Task
“wait until production is proven” field may survive this cut.

## Alternatives and review findings

- **File before merge:** easier with current async land, but violates Jack's
  chosen post-land step and can create follow-through for code never shipped.
  Preserve suggestions in PR copy first; commit filings after verified merge.
- **Have reconcile launch an agent:** makes completion look automatic but turns
  read/recovery into an unrequested execution service. Keep autonomous work in
  authored Flows and existing operators.
- **Keep land async and add a mandatory review node:** exposes the gap but asks
  Jack to click through routine filing. Prefer the existing ship edge waiting
  for merge and finishing its work. Unknown scope still reaches the conversation.
- **Leave the old Task open with a deadline:** already implemented by #1488 and
  directly superseded by Jack's decision. Transfer the obligation, not its wait.
- **Copy all parent scratch:** creates conflicting current-slice markers and
  restores the failure from LOO-394. Explicit child-specific handoff preserves
  accepted context without inheriting another Task's working notes.

Simulated design review changed the proposal in three places: checkout placement
must survive without a PR; filing retries must retain their original Project
across chapter rotation; and a merged PR must stay visible as pending until its
filing receipt is confirmed. These are failure boundaries, not extra approval
steps. Success means finishing becomes boring and later evidence has an owner.
Failure would be a new queue nobody visits, duplicate follow-ups after outages,
or hidden multi-PR state behind a simpler label; the acceptance scenarios below
exercise those exact cases.

## Internal slices and acceptance

One coherent lifecycle landing for LOO-418; internal slices do not authorize a
serial PR chain. If independent work is split later, each additional PR needs
its own Task and child-specific design. The original four slices and the independent completion trigger are implemented locally.
Shared planning integration, combined proof and complete demo remain; these are
internal slices of the same delivery boundary.

**Remaining work:** integrate LOO-406's common local planning writer once its
coherent boundary is ready, then finish the unified lifecycle acceptance population
below and Jack's complete demo. The current completion writer still contacts
Linear before settling local status; this must be replaced by the common local-save
and optional-sync owner before delivery, never retained as a second implementation. The affected
gate ran against the earlier model; its failures have passing focused repairs.
Those results do not verify the newly accepted completion contract.
No-PR research now retains dirty files and committed findings; merged Tasks need
the filing disposition. The implementation defaults below are reversible choices,
not additional decisions attributed to Jack. Disposable Docker installation checks
passed; configured/native acceptance is absent and landing is unauthorized.

1. **Truthful Flow launch.**
   `task_flow_launch_tests` holds a real mechanical Flow in a disposable Home:
   Task run from a sibling, direct `--task`, plain checkout, symlink and `--wt`
   all show target Started and current Flow in status and inventory, retain
   caller ancestry, and finish successfully. Passive inspection leaves Started
   absent; failed placement remains an observed failed Process. Store tests
   prove atomic refusal and explicit Session membership after checkout removal.
   Independent completion tests replace the original alias assertions while
   preserving retained input and idempotent retry.
   No provider or native Desktop acceptance follows from these fixtures.
2. **Optional PR and Task-owned placement.** Rotation and placeholders are
   removed; CLI/Swift expose one optional PR and migration retains historical
   rows. Research completion retains committed findings and dirty files; an
   open published PR still blocks successful completion.
3. **Linked follow-through.** Durable filing/retry, dates, waited landing,
   finishing Flows and pending/done projections replace keep-open obligations.
   The skill and operators must use the independent completion operation. Separate fixtures
   cover filing and completion; the unified lifecycle proof below remains.
4. **Selected design handoff.** Checkout transfers an explicit child design;
   retries preserve edited/deleted child notes and conflicts retain both inputs.
   `task_handoff_tests` covers those boundaries; existing `sync_tests` covers
   parent changes and squash integration. The complete stacked demo remains.

5. **Independent completion and end trigger — locally implemented.** Status has
   a durable completion mark independent of position. End stores its request with
   arrival; failed completion retains both. Fresh provider completion/reopening
   preserves execution and supersedes old requests. CLI, shared projections,
   Desktop, instructions and the one migration draft use the new contract.
   Integration must move settlement onto LOO-406's common local writer.

Gate runs the changed-aware headless plan once:

```sh
uv run python scripts/test.py --list
uv run python scripts/test.py
```

Its Rust/Swift/format/Clippy plan must include the affected scenarios below;
add missing coverage to existing suites rather than relying on a zero-test
filter. Use the released-frontier installation harness for populated upgrade:
`uv run python scripts/test_task_installation.py`. Its disposable Docker installation
is available locally and in capable CI; it never targets an installed Machine. Rust command-tree changes
also run `cargo test -p loopflow --lib engine::flow_graph::tests`.

| Observable result | Headless proof owner |
| --- | --- |
| Normal delivery completes a merged Task only after follow-through is resolved; CLI, work monitor and Desktop show independent status, Workflow position, PR and links | Extend `task_flow_launch_tests`, `land_tests`, DTO fixtures and `TaskFlowProofTests`/`RegistryQueryTests` with the same lifecycle population |
| `task complete` changes status without moving Workflow; move/arrival at end triggers the same operation; completion inside follow-through followed by driver arrival records one completion and actual movement | Task launch, authority and command-tree tests; shared Rust/Swift DTO/view population |
| Accepted Linear completion while a real held Process is running preserves its Process identity, liveness, Workflow edge, Session and checkout; later driver exit records its true result | Public Task/Flow case plus PM observation tests and CLI/monitor/Desktop projection |
| End is durable before a failed completion; crash/retry finishes only completion, keeps successful Flow history, and exposes the reason; repeated completion is harmless | Task launch, store transactions and operator guidance; verify gate/Flow invocation counts |
| Newer Linear reopening changes status without Workflow movement; old end and pending retries cannot re-complete it; stale provider observations do not undo newer state; reopening between outbound read and mutation survives | PM revision/writeback and completion trigger tests, including LOO-406's enabled failing concurrent-reopening regression |
| Real Git fixture plus simulated GitHub merge and Linear mutation crosses store, public CLI JSON, monitor projection and Swift decode/view; no live provider or display is required | New lifecycle case in existing Task launch suite; serialize its resulting wire fixture for both Rust `dto_fixtures` and Swift `DTOFixtureTests`/headless Task view assertions |
| Lost creation response, crash before local receipt, simultaneous finishing callers, issue edited/moved and chapter rotated all reuse one child; provider failure leaves explicit pending state | PM/Linear tests, `task_pr_authority_tests` and Task launch integration; assert issue population and Task state, not mock calls |
| Bare land, waited land and out-of-band merge all converge; wait timeout/interruption does not clear intent, replay gate or complete early; reconciliation between merge and filing retains the checkout for the next step | `land_tests`, `pr_landing` tests and a public CLI held-Flow case |
| Research reaches done with `pr: null`; retained drafts/commits and Sessions stay reachable; no hidden Working PR is created | `task_initialization_tests`, `task_diff_tests`, Task launch and shared DTO/view fixtures |
| Child publishes before parent merge, accepts parent updates and survives squash with one PR and its own design; changed handoff input never overwrites child edits | Existing `sync_tests` stack scenarios plus `task_handoff_tests` |
| Due tomorrow is absent from today's due list, present on the next due Wave pass, and never auto-completes; unscheduled coverage is stated honestly | PM projection/operator prompt scenarios with fixed dates; Desktop/CLI use the same due date |
| Populated zero/one/multi-PR and old remaining-work records retain IDs, active code, pending obligations, history and provider writeback under upgrade | `store::migrations` plus disposable installation harness |

Fixtures prove the shared contract, not sustained use. Jack's demo review judges
whether the Workflow and Task page make sense. Product's chapter KRs still need
three working days of primary Desktop use and three two-hour sessions without
crash or access loss; this design claims neither. No new performance metric is
needed for these bounded reads and existing UI surfaces. Gate should verify
that ordinary Task reads add no per-follow-up subprocess or network call.

## Implementation evidence (reconciled 2026-10-08)

The committed implementation includes Task-owned checkout placement,
optional current PR with immutable historical rows, post-merge disposition and
UUID-pinned Linear filing, waited land/finishing Flows, shared due/link projections,
and explicit design handoff. Focused implementation checks pass. The earlier
inventory records the starting point; the affected gate ran with focused repairs.
The combined lifecycle proof and Jack's complete demo remain.

Review removed serial store settlement and PR-based placement writers as well as
CLI rotation. Task placement writes update the validated PR copy in one direction.
Publishing now separates reserved intent from confirmed reviewer copy, preserving
the prior copy when draft promotion fails. Historical continuation followed by an
unpublished placeholder requires explicit scope conversion. No-PR completion keeps
its artifacts, and merged completion checks the disposition inside its transaction.
Legacy scope notes retain the original outcome, evidence condition, date and reason
in CLI/Swift projections; a conversion flag alone did not give the finisher enough
context. Reconcile observes a merge successfully while follow-through is pending.

The public waited-landing test found competing Ctrl-C handlers. The existing
process handler owns interruption (130, stopped without retry); timeout is held
(3). Its lifecycle proof retains the same PR/request and checkout across interruption,
retry, authoritative merge and later reconcile. Detached repair fixtures clear
inherited `LF_BIN`: otherwise their children silently use the installed binary.

Filing uses `--key` (default `follow-up`) for stable obligation identity; a second
obligation needs a distinct key. Retries retain the original UUID, relation UUID,
Project, Team, initial state and exact content. This is an implementation choice,
not a new decision attributed to Jack. The operation-level fixture loses both
mutation responses and readbacks, rotates the chapter and edits/moves the issue,
then confirms the original receipt and exactly one provider issue/relation.
The CLI proves no-PR retained research and post-merge completion within a Flow;
Swift fixtures prove pending/done links and legacy scope text. These separate
proofs do not establish a single configured provider-to-monitor-to-native run.
The unified lifecycle population remains unproven. The real wait loop now has
paused-clock timeout proof, and the disposable Docker installation checks passed.
The initial Docker pass silently selected zero Flow-history tests: removing its
incorrect `--ignored` selection exposed and passed the actual test.

## Implementation defaults and delivery boundary

Jack's zero-or-one PR and normal post-merge follow-through contract remain
accepted. October 8 supersedes the alias: end triggers independent completion,
and Linear completion preserves Workflow and Process state. The current code
separates completion and position, but still needs the shared local planning
writer and composed lifecycle proof before the complete demo on the existing PR.

The implementation uses the proposed polling limits and filing interface,
removes `-c` and `--next`, and provides explicit `--design` handoff. Dated follow-ups return on the owning Wave's next operation; filing
does not install a schedule, and unattended execution requires a concrete check
already authorized in the brief. These are reversible implementation choices,
not separate approvals attributed to Jack. Their review belongs in the complete
demo, with the liveness, retry, history and migration protections above intact.

PR #1499 remains the single delivery boundary. Live Home migration, closing LOO-385,
automatic filing of unrelated improvements, landing this branch and claimed
production acceptance remain excluded. Gate findings and remaining acceptance
are recorded below; the Docker installation ran in isolation.

Review (2026-10-07): the inverse Flow lookup uses indexed Process-bound Session
rows and shared membership predicates, avoiding a scan of all Process history
for each Task. That earlier alias added no independent state; October 8 supersedes it. The suspected combined
`--wt`/`--task` cwd-guard issue is unreachable because Clap rejects the pair;
no extra guard remains. Fixture corrections selected an authored workflow edge,
used `flow list --processes`, and decoded the actual flattened inventory DTO.
This earlier review deferred affected checks to gate; the October 8 results
below supersede that deferral. Rendered judgment remains with demo.

Compression keeps placement as a returned binding and the driver as the sole
holder of its Task during execution. Flow registration takes its name from the
captured graph, removing a second input that could disagree. Historical fixtures
retain their test-only Started writer because they distinguish retained Flow
history from admission; production writes Flow and Started together.
Workflow moves now share one Task lookup and omit the unused repository argument.
The single-PR projection no longer selects a “latest” PR or compares it with itself.

Synced main at `812d8cc55`, retaining placement before Process admission and
applying main's `cli.agent` rename there. Earlier focused lifecycle, Rust/Swift DTO,
filing, placement, stack and migration evidence is retained at
`89e1198c5:scratch/make-a-task-up-to.md`.

Review (2026-10-08): confirmed follow-up links incorrectly copied the immutable
filing date instead of Linear's observed due date. Confirmation now records the
provider date, including removal; the retry receipt retains its original payload.
The lost-response/changed-chapter regression covers both date edits and removal.

Range verification now reads Task placement directly, without reading or creating
a temporary PR. Completion reconciliation uses its existing gate once; the store
still checks disposition transactionally. Filing reuses its initial PR read.
Review also corrected the CI repair prompt's completion promise and the Desktop
mock's malformed optional-PR JSON; its existing decode test proves the repair.
Earlier post-sync checks are retained at `6d18dad0a:scratch/make-a-task-up-to.md`.

October 8 review corrections: waited landing shares CI repair admission with the
watcher and release. Recorded Flow drivers and the Task carrier waiting for the
calling command do not block its repair; other live work and pending provider
turns still do. A busy checkout is reported without persisting a false delivery
failure that could stop another waiter. The combined CLI fixture proves repair
without a watcher, refusal over unrelated work, a watcher's harmless busy pass,
one repair and arrival at follow-through. The mock changes CI evidence; it is not
a live provider repair. Initial fixture failures were missing canonical planning
scope and an invalid mock GitHub branch response, both corrected in the fixture.
Held exit 3 now survives the Flow driver and prevents automatic whole-Flow retries.
Bare and waited land return success for a merged Task PR, including a completed
Task and explicit worktree selection, without clearing newly written scratch.

Gate review (2026-10-08): a blanket error conversion made genuine waited-landing
failures held, suppressing Task-runner retries. Only timeout, deliberate blockage,
closed-without-merge and checkout-busy results now stop without retry; storage
failures remain failures. The real loop's timeout retains its original request.
A second defect joined every mechanical step to recent landing timestamps: after
a hold, an unrelated first command could stop a fresh Flow. Only the parsed
nonwaiting land command now checks pending delivery at its resolved checkout;
short command spellings and explicit worktree selection use the ordinary parser.
The regression retains an old request with a later observation timestamp.

Other gate failures came from obsolete fixtures: missing explicit nullable due
dates and workspace base commits, PR placeholders in research completion, old
placement writers and command inventories, and expecting confirmed presentation
from a failed publication. The populated-upgrade test now starts before the Task's
draft instead of applying the draft twice. No production invariant was relaxed.

The prior gate's exact results and repaired failures remain at
`37441026a:scratch/make-a-task-up-to.md`; they cover the predecessor model.

Implementation review (2026-10-08): status reads previously reconciled completion,
so merely inspecting a Task could settle its pending request. They now read
without executing that trigger. Desktop's Done label also hid unresolved
follow-through; it now retains that obligation, and history filtering keeps
unresolved delivery visible even with no live Process. A running Flow keeps its edge
after accepted provider completion and records its real arrival. Newer reopening
clears superseded intent; PM delivery checks the request after fresh acquisition.
Migration seeds historical completion and retains pending provider writebacks.
No new Task type, watcher or worker was introduced.

The historical `scratch/pr-review.html` retains revision-pinned excerpts from the
superseded alias model. Its notice now distinguishes the implemented trigger from
remaining integration; it is not a walkthrough of the current contract. A current
walkthrough remains part of the complete demo preparation.

The current public offline fixture deliberately cannot write Linear: it verifies
pending completion, then ingests accepted provider status through the normal
store writer. Stateful provider fixtures independently prove successful completion,
lost responses and reopening. Together with the live held Process and atomic
arrival/store-reopen cases, these cover local trigger and observed-revision behavior;
they do not establish concurrent provider-write safety or compose post-merge filing,
local-only planning, monitor and Desktop in one population.

Earlier completion-trigger checks and repaired expectations are retained at
`5c240c89a:scratch/make-a-task-up-to.md`. They do not prove the unfinished local
planning integration or unified lifecycle.

Check (2026-10-08): reused the unchanged-code result with inherited `LF_*` cleared, `cargo nextest run -p loopflow --lib --test task_flow_launch_tests --test task_initialization_tests -E 'binary(task_flow_launch_tests) | binary(task_initialization_tests) | test(ops::task_actions::) | test(lf::commands::waves::)' --no-fail-fast` — 53 passed, formatting/Clippy passed; prose-only realign: `git diff --check` and `lf context --skill realign --json` passed within budgets. Full affected gate, LOO-406 integration, unified lifecycle proof and configured demo remain; LOO-406's recorded failing regression was not rerun.

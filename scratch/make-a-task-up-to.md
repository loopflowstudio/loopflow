# One Task, up to one PR

LOO-418 · integrated local planning at `314095b00` · reconciled 2026-10-08

Jack Heart authorized stacking on LOO-406 and continuing. The merge is recorded
at `9f54c5b43`, with repairs at `07eeef61d` and focused Docker proof.
Common local creation and state delivery replace the separate provider-first
filing/completion writers; `1fe22f565` adds relation export and `314095b00`
simplifies completion and shared reads.
Task completion remains independent of Workflow position and Process liveness.
The earlier evidence below describes pre-integration populations; it does not
verify the combined branch. Full gate, stacked/due-return proof and Jack's demo
remain. Landing and Task completion are unauthorized.

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

Jack Heart requested one Task model and explicit `--wait-and-fix` naming. Bare
land still returns after requesting merge. The committed parent `558a39232`
supersedes earlier observations of an unavailable common writer at `de000c24f`.
The parent checkout was not changed.

The merge keeps Task, TaskPlan and the assembled TaskSnapshot, optional Linear
mapping and checkout, and one optional PR. Placement saves without a placeholder
PR; status exposes absent branch/base until checkout. Completion writes local
planning state and its optional delivery receipt in one transaction, retaining
LOO-418's separate durable end request. Workflow movement does not reopen or
complete planning by alias. Provider observations preserve Workflow and Processes
and supersede old completion requests. No provider-first completion writer remains.

Follow-through now reserves a local Task identity, then uses the common NewTask
writer. Its receipt retains the original Project, description and due date; normal filing
and local links need no provider. Reservation and common local creation now commit together; old reservations
recover in their pinned Project even after selection changes. Normal planning export owns eventual Linear creation.
Historical provider receipts can acquire an already-created issue without issuing
a duplicate creation. The local disposition and optional Linear relation are
separate effects. Foreground planning export now delivers eligible relations once
both endpoints are mapped, including after source completion. Confirmation is an
idempotent Task event; lost replies retry the original relation UUID. Filing no
longer calls a separate relation sender. Reads resolve links against current
saved Task names, URLs and dates, without changing immutable filing input.

Integration update (2026-10-08):

- Reservation and child creation share the common transactional writer. A failed
  reservation leaves neither child nor receipt. Existing reservations recover the
  original child, payload and Project after rotation; unrelated old-Project
  creation still refuses. The separate post-reservation creator is deleted.
- Historical provider intents convert into the common local creation/export
  receipt, retaining the UUID, Project, Team, state, description and due date.
  Saved filing input remains immutable. Common receipt aliases resolve local
  navigation before mapping; exact provider readback adopts the same child.
  **Remaining uncertainty:** the predecessor saved no network-attempt boundary.
  Conversion conservatively retains an attempted/unknown export. If the issue is
  absent remotely, it stays pending rather than issuing a potentially repeated
  create. Local filing can finish; automatically delivering an uncreated historical
  issue still needs evidence or a reviewed retry rule. No second creator is added.
- `lf task reopen ISSUE [--reason TEXT]` uses common planning state delivery,
  clears superseded completion intent, and retains PR, Workflow, checkout and
  Processes. It starts nothing and does not restart the Workflow. The spelling is
  an implementation choice, not a new decision attributed to Jack.
- Combined CLI/Desktop lifecycle, populated released-data upgrade, stacking and
  due-return acceptance remain with gate; Jack's complete demo remains separate.

LOO-406's design records Jack Heart's Linear-wins policy for observed conflicts.
Its enabled `task_completion_preserves_linear_reopening_during_delivery` regression
retains the unseen-write race: an edit between read and unconditional mutation can
be overwritten, and matching readback cannot prove preservation. Conditional-write
research does not block source integration. Neither the merge nor local acceptance
claims that race is fixed. The known failure remains in acceptance, not an
ignored test. Linear-wins precedence supplies no atomic-write guarantee; any
relaxation of concurrent-edit preservation still requires an explicit decision.

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

## Inventory and findings

The original symbol-by-symbol inventory, including LOO-385 overlap and retired
serial-PR/completion paths, is retained at
`07eeef61d:scratch/make-a-task-up-to.md`, “Inventory and findings.” The surviving
ownership and deletion cut are below. The inventory predates common local planning
and supplies neither combined acceptance nor authority to close LOO-385.

### Launch and provider findings

The shared launch defect was admission at the caller's cwd before Task placement.
The implemented cut resolves performed location first and atomically records Flow
and Started. The original SQLite probe, CLI population and Linear schema sources
are retained at `07eeef61d:scratch/make-a-task-up-to.md`, “The launch defect has a
shared cause” and “Provider support.” Live provider writes remain unproven;
observed due dates and immutable retry input remain separate.

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
Project or use a changed title as a new request. Confirm the saved Task and local
link before recording filed; optional provider export proceeds independently. Repeated calls reuse receipts, including after a crash
between provider success and local writeback. A second deliberate obligation
gets a different request key. Never hold a local lock across an agent turn.

Local planning owns the follow-up's authored plan and date; observed conflicts adopt Linear. A native `related` relation
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

Use the saved planning due date, exported to Linear when connected, for the day
a check is wanted. Exact release prerequisites,
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

Integration removed `complete_planning_task`, `task_pm::complete_task`,
`reconcile_pm_writeback`, the completion arm of `PmTaskUpdate`, and direct
completion writeback updates. Common local planning owns creation and status;
follow-through owns its linked disposition. No parallel provider-first writer remains.
The inline relation sender in `confirm_intent` is deleted; foreground planning
export owns that optional effect. The obsolete `with_pr` fixture switch is removed:
placement alone creates no Working PR, and tests must publish explicitly.

Compression removes the unreachable provider-error fallback and absent-execution
mapping in `task_status`, and the impossible absent result of `task_complete`.
Completion reads Task state once inside its immediate transaction; the request,
merge and disposition checks still happen there. `FollowThrough::links_confirmed`
owns the shared filing test. Filing reuses its open store, and `follow_up_sources`
joins target aliases in its single inventory read instead of querying per link.
Historical missing targets and repeated link presentations retain their source.
The skill and delivery guide no longer require provider relation confirmation
before local disposition. Historical conversion now uses the common receipt;
unknown remote creation stays pending. No separate creator is restored and the
provider race remains unresolved.
Planning-lookup fixtures exposed two reads outside their scoped store: Wave discovery
and saved Wave configuration. Discovery now uses the existing PM selector, and
the test-only configuration path reads that fixture’s saved documents, not the
default Home. The first two runs failed 16/17 and 13/17 cases respectively; the
corrected population passes without changing production Wave ownership.

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

`07eeef61d:scratch/make-a-task-up-to.md`, “Alternatives and review findings,”
retains rejected pre-merge filing, reconcile-launched agents, mandatory finishing
reviews, keeping the source open, and wholesale parent-scratch copying. Keep
Task-owned placement, pinned retry destinations and visible pending follow-through;
simulated design review established those failure boundaries, not Jack's approval.

## Internal slices and acceptance

One coherent lifecycle landing for LOO-418; internal slices do not authorize a
serial PR chain. If independent work is split later, each additional PR needs
its own Task and child-specific design. The original four slices and the independent completion trigger are implemented locally.
The common planning integration and crash/local-reopening repairs exist;
historical remote-creation uncertainty, combined acceptance and the complete
demo remain within that same boundary.

**Remaining work:** resolve the historical remote-creation uncertainty above,
run the unified lifecycle acceptance population below and Jack's complete demo. Completion and filing now
use local planning; the earlier evidence does not verify the combined source. The affected
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
   The skill and operators must use the independent completion operation. The composed provider fixture now covers filing, independent completion and actual
   arrival through CLI/monitor; combined Swift proof remains.
4. **Selected design handoff.** Checkout transfers an explicit child design;
   retries preserve edited/deleted child notes and conflicts retain both inputs.
   `task_handoff_tests` covers those boundaries; existing `sync_tests` covers
   parent changes and squash integration. The complete stacked demo remains.

5. **Independent completion and end trigger — locally implemented.** Status has
   a durable completion mark independent of position. End stores its request with
   arrival; failed completion retains both. Fresh provider completion/reopening
   preserves execution and supersedes old requests. CLI, shared projections,
   Desktop, instructions and the one migration draft use the new contract.
   Settlement now queues local state and optional provider delivery in the same
   transaction; combined acceptance remains.

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
| Real Git fixture plus simulated GitHub merge and Linear mutation crosses store, public CLI JSON, monitor projection and Swift decode/view; no live provider or display is required | Composed case in `ops::task::follow_through::tests::lifecycle`, with a real CLI/Flow and simulated providers; its captured wire population is checked by Rust `dto_fixtures` and Swift `DTOFixtureTests`/headless Task view assertions |
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

## Earlier implementation evidence

`07eeef61d:scratch/make-a-task-up-to.md`, “Implementation evidence,” preserves
pre-integration checks, failures and repairs. They cover publication copy,
retained artifacts, interruption ownership, filing identities and the corrected
zero-test Docker filter, not the combined model. Current integration evidence is
below; configured provider/native acceptance remains unproven.

## Implementation defaults and delivery boundary

Jack's zero-or-one PR and normal post-merge follow-through contract remain
accepted. October 8 supersedes the alias: end triggers independent completion,
and Linear completion preserves Workflow and Process state. The current code
separates completion and position through the shared local planning writer.
Combined acceptance and the historical export decision precede the complete demo.

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

Earlier compression, sync and gate repair details remain at
`432f3706575543c981a1a52da58ef4ce4874443f:scratch/make-a-task-up-to.md`,
“Implementation defaults and delivery boundary.” That history retains failed
fixtures and their corrections: observed due dates versus immutable filing input,
confirmed publication copy, waiting-controller repair admission, held versus failed
landing outcomes, and preventing unrelated mechanical steps from adopting a prior
landing. Those checks cover the predecessor model; current composed evidence and
remaining integration obligations are below.

Implementation review (2026-10-08): status reads previously reconciled completion,
so merely inspecting a Task could settle its pending request. They now read
without executing that trigger. Desktop's Done label also hid unresolved
follow-through; it now retains that obligation, and history filtering keeps
unresolved delivery visible even with no live Process. A running Flow keeps its edge
after accepted provider completion and records its real arrival. Newer reopening
clears superseded intent; PM delivery checks the request after fresh acquisition.
Migration seeds historical completion and retains pending provider writebacks;
the inherited local-planning draft and this Task's optional-PR draft remain
separate Task-owned drafts, not two revisions of this Task's schema.
No new Task type, watcher or worker was introduced.

The historical `scratch/pr-review.html` retains revision-pinned excerpts from the
superseded alias model. Its notice now distinguishes the implemented trigger from
remaining integration; it is not a walkthrough of the current contract. A current
walkthrough remains part of the complete demo preparation.

## Composed headless lifecycle

The pre-integration population and failed fixture attempts are retained at
`07eeef61d:scratch/make-a-task-up-to.md`, “Composed headless lifecycle.” It held a
real Flow across simulated merge, in-process filing, independent completion and
actual driver arrival. One Home supplied CLI, roadmap and monitor JSON, captured
in `tests/fixtures/dto/task_lifecycle.json` for Rust/Swift decoding. This remains
simulated-provider proof, not autonomous filing judgment or native acceptance.
Its subsequent compression kept settlement's transactional recheck. Integrated
checks below do not erase the earlier failures or prove the concurrent-write guarantee.

Implementation review (2026-10-08): foreground export now delivers related-issue
links from retained filing intents after either endpoint gains its mapping. A
confirmation event prevents continuous remote rereads. Source completion is
independent; reads resolve displayed names/URLs/dates from saved Task values.
The inline filing sender is deleted. The simulated provider test loses creation
and relation responses/readbacks, removes the child's date, and recovers one
issue/relation without reopening the source or completing the child.

All-target compilation exposed integration fixtures still assuming mandatory
placement and a Working PR. They now unwrap explicitly placed fixtures, establish
an actual published parent, and assert an unplaced Task rather than no Task row.
The first new test also assumed a Task-owned Wave column and omitted its isolated
Home; both fixture/query errors were repaired. An overly broad test-name filter
selected two Git cherry-pick tests that failed on the Docker image's older Git
option set; they remain with capable gate/CI, not silently counted as passes.

Check (2026-10-08, disposable Docker): focused follow-up export/lost-response test, research checkout, due-follow-up projection and four handoff tests pass; prior two unchanged local/composed lifecycle tests pass; `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `git diff --check` pass. The integrated unseen-write reopening regression remains failing as recorded at `07eeef61d`; full gate, Swift and populated installation checks remain with gate.

Check (2026-10-08, compression): disposable Docker `cargo test -p loopflow --test task_follow_through_tests` (3), `--lib ops::task::follow_through::tests` (3), `--lib end_request_is_atomic_and_survives_reopening_the_store` (1), and `--lib ops::pm::planning_lookup_tests` (17) pass; `cargo clippy --all-targets -- -D warnings`, native `cargo fmt --check` and `git diff --check` pass. Native compilation stalled in the build-script loader and was stopped; Docker supplies the build proof. Full gate/Swift/installation and Jack’s demo remain with their existing owners.

Implementation review (2026-10-08): old filing events cannot distinguish a
never-sent request from a lost creation reply. Conversion therefore preserves
uncertainty in the common export receipt instead of treating absence as permission
to send. Atomic local filing initially failed three existing tests because the
selected Project projection can use its provider ID; admission now resolves that
alias to the stable local Project before checking selection, and new receipts pin
the local identity. The first reopening test exposed that common state delivery
supports unstarted, not started: reopening now returns planning to unstarted while
preserving Started and execution. No schema extension is needed. The enabled
unseen-write reopening regression remains unresolved and unchanged. The held-Flow
fixture also exposed its predecessor assumption that adding a Workflow file after
Wave import changes the catalog. Renaming it and changing its checkout did not
fix that; the fixture now stores the definition through `project workflow set`.

Check (2026-10-08, integration recovery): Docker `cargo test -p loopflow --lib ops::task::follow_through::tests` (5), `--test task_follow_through_tests` (3), `--test task_flow_launch_tests` filtered to local reopening and live-edge preservation (1 each), `--lib end_request_is_atomic_and_survives_reopening_the_store` (1), `--lib engine::flow_graph::tests` (10), and `cargo clippy --all-targets -- -D warnings` pass; native `cargo fmt --check` and `git diff --check` pass. Combined gate/Swift/installation and Jack's demo remain; the enabled concurrent-reopening regression is unchanged. `lf commit` returned exit 1 without a diagnostic; prior scratch edits were preserved before editing.

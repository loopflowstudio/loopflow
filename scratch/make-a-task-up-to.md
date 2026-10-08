# One Task, up to one PR

LOO-418 · four slices implemented; gate repairs checked; combined lifecycle proof and demo remain · reconciled 2026-10-08

## Decision and intended experience

Jack decided on October 7 that a Task delivers zero or one PR, never a serial
chain. Follow-through is a follow-up Task: file it after landing, then complete
the landed Task. Dependent implementation starts in another stacked Task before
its parent merges. A recommendation can finish without a PR.

Jack Heart confirmed the completion rule during design review on October 7:
`PR merges → file follow-ups or record “none needed” → Task completes`.
Jack then clarified that no Task at `end` may remain incomplete. Restore
`lf task complete ISSUE` as an alias for `lf task move ISSUE end`, sharing one
transition and its checks. Jack requested continued advancement, then challenged
the premature demo stop before the larger lifecycle changes were built. The
launch/alias milestone was the agent's scope reduction, not Jack's requested
boundary. The full remaining lifecycle is implementation scope on the same
PR #1499. Demo follows the complete change; landing remains unauthorized.

The experience: a Task has one outcome, an optional PR link, and one Workflow
position. After delivery, its page says either **Done · Follow-up LOO-…**, or
**Merged · Follow-through pending**, with the reason and next action. It never
says “PR 2,” retains an unexplained open state, or waits for production evidence
that belongs to another Task. Research reaches `end` without a phantom PR slot.

## Alignment with local planning (2026-10-08)

Jack Heart requested avoiding duplicate data types and making repair explicit in
`--wait-and-fix`. The landing flag replaces `--wait`; ship and all callers use
that spelling. Bare landing still returns after requesting merge.

LOO-406 now implements the complete local planning lifecycle. Its October 8
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
The current follow-through implementation still calls Linear directly; merging
the DTOs alone will not make that path work without Linear. Verify unplaced,
PR-less and merged-with-follow-up Tasks through both CLI and Desktop, including
local filing/completion without provider calls. No second filing or completion
implementation should survive. LOO-406's active checkout was inspected read-only.

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

The four lifecycle slices and October 8 landing corrections are implemented
locally. A Task/Flow/waited-land fixture now repairs CI and reaches follow-through;
repeated landing preserves finishing notes for pending and completed Tasks.
The complete lifecycle proof and Jack's demo review remain.

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
`--wait` for callers that own a blocking Flow. It uses the existing finite
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
The enclosing Workflow's subsequent arrival at end is idempotent: one completion,
one history transition and retryable provider writeback. No new Workflow review
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
run. After the disposition reaches end, existing cleanup may retain a checkout
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

### 3. End means completed delivery; a PR is optional

Keep one Task status derived from Workflow position: start/ready,
between/active, end/done; abandoned remains its separate mark. No new “landed
Task” state or stored `done` flag. Merge and follow-through are delivery facts
shown alongside that position.

**Jack Heart's clarification, 2026-10-07:** `end` and completed are the same fact.
Restore `lf task complete ISSUE` solely as an alias for `lf task move ISSUE end`.
Both enter the existing `task_end`/`reach_end` operation, as does automatic
arrival at end; neither spelling can bypass the completion checks. If a check
fails, preserve the current Workflow position. A repeated completion succeeds
without another move or duplicated filings. No completion flag, intermediate
"at end but incomplete" state, or Session ready/complete API is added.

For a Task that has a PR, end requires authoritative merge plus a durable
none/filed disposition. An open or closed-unmerged PR cannot become successful
delivery. Explicitly abandoning the Task remains available. Reopening a closed
PR reuses the same PR; a different PR requires a different Task. Reopening the
Task for a correction never grants another PR slot.

For a Task with no PR, an accepted empty edge or explicit `lf task complete ISSUE`
completes it. Commits and files can be research artifacts; their presence does
not manufacture a PR requirement. Preserve its checkout whenever deletion would
lose work. Completion and physical cleanup remain separate, as in #1488.
Inspection, process liveness uncertainty, or retained Sessions do not veto a
valid completion. Findings that need later verification can themselves file
follow-ups, but no-PR Tasks do not acquire a mandatory landing ceremony.

Move branch, base commit and stack placement from the always-created `TaskPr`
onto Task's checkout placement. File browsing and checkout restoration use that
placement even without a PR. Create the one optional PR record on first publish,
with its publishing intent durable before the remote call. Publication retries
find the same PR. Keep GitHub number/head/merge/checks in that PR record, not
copied onto Task. Shared status replaces `prs`/`active_pr` with `pr: Option<…>`;
normal screens and launch context say “Pull request” and omit it when absent.

Delete `land -c` and `--next` (also arm/submit variants). Bare land has one
meaning; `--wait` changes waiting, not completion policy. Taskless PR delivery
continues without inventing a Task or follow-up. `task complete` and `task move end`
use the same delivery facts as automatic arrival; `--force` must not bypass merge or required
filing. An early Linear auto-completion is reported as provider conflict, then
reconciled to the actual Workflow state, never adopted as local completion.

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
foreign keys to old PR IDs. Completed historical Tasks remain complete.

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

These cuts are implemented together in slices 2–4; retained historical readers
are the explicit migration exception:

- `PrCommand::Next`, next-slug CLI fields, `AfterMerge` runtime branching,
  rotation-only helpers listed above, and serial-only tests/copy/fixtures.
- The live `TaskFollowUp` keep-open writer, `task_follow_up_resolved`, deadline
  blockers and `--clear`; keep deserialization of immutable historical events.
- Placeholder creation on checkout, PR-dependent file/restore paths, and
  `CompletionGate.discardable_successor` once no working placeholder can arise.
- `TaskSnapshot.prs/active_pr`, ordinal PR chips and “PR 1” launch copy. Update
  Rust/Swift DTO fixtures together, including explicit null for no PR.
- `land -c`/serial guidance in LOOPFLOW.md, landing/capture/ship-decomposed and
  operator skills, `docs/lf*.md`, delivery architecture and affected READMEs.

The compression review also removed the surviving Swift `startNextPr` action,
keep-open summary/reader, and disposition-aware repair-command helper. Writers
leave retired disposition columns untouched; historical events still feed the
shared follow-through projection. No further deletion target is currently known.

Retain exact-head landing generations, CI repair deduplication, Git mutation
locks, safe cleanup, Task history, shared status/monitor reads, symlink-safe file
access, and existing stacked sync. No new Flow primary selector, resume
controller, Session approval API, second Task completion flag, or source Task
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
its own Task and child-specific design. All four slices and the completion alias are implemented locally. The combined
lifecycle proof and complete demo remain; the internal slices are not separate delivery boundaries.

**Remaining work:** the unified lifecycle acceptance population below, LOO-406
integration once its active work is ready, then Jack's complete demo. The affected
gate ran; its failures have passing focused repairs.
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
   `task complete` shares `workflow_set(..., end)` with move; tests cover
   refusal without movement, force/reason, retained input and idempotent retry.
   No provider or native Desktop acceptance follows from these fixtures.
2. **Optional PR and Task-owned placement.** Rotation and placeholders are
   removed; CLI/Swift expose one optional PR and migration retains historical
   rows. Research completion retains committed findings and dirty files; an
   open published PR still blocks successful completion.
3. **Linked follow-through.** Durable filing/retry, dates, waited landing,
   finishing Flows and pending/done projections replace keep-open obligations.
   The skill and operators use the shared completion alias. Separate fixtures
   cover filing and completion; the unified lifecycle proof below remains.
4. **Selected design handoff.** Checkout transfers an explicit child design;
   retries preserve edited/deleted child notes and conflicts retain both inputs.
   `task_handoff_tests` covers those boundaries; existing `sync_tests` covers
   parent changes and squash integration. The complete stacked demo remains.

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
| One merged Task becomes end/done only after its zero-or-more follow-ups are durably resolved; CLI, work monitor and Desktop agree on PR, state and links | Extend `task_flow_launch_tests`, `land_tests`, DTO fixtures and `TaskFlowProofTests`/`RegistryQueryTests` with the same lifecycle population |
| `task complete`, `task move end` and automatic arrival produce the same end/done result; refusal leaves position unchanged; completion within follow-through followed by driver arrival or retry produces one transition | Task launch integration, Task authority tests and command-tree coverage; operator prompt scenarios include merged but unfinished Tasks |
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

Jack's zero-or-one PR and post-merge follow-through check are accepted: file
follow-ups or record none needed before completing every merged Task. Jack also
accepted the landing Flow/operator recovery approach and clarified that end is
completion, with `task complete` an alias for moving to end. Jack's correction
keeps the full lifecycle under implementation before demo, on the existing PR.

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
for each Task. Completion adds no authority or state. The suspected combined
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

Check (2026-10-08, inherited execution variables cleared): `scripts/test.py --reuse-passing` initially reported 2,229/2,295 Rust and 401/402 Swift passing; all 66 Rust failures recovered in focused materialized reruns (the upgrade case moved to the migration suite), the Swift regression passed, and 49 affected Flow/Task-launch tests passed. Architecture, website, Swift CLI build and multiplatform checks passed; Docker installation ran all seven named proofs plus planning upgrade after correcting one zero-test invocation. Final two landing regressions, `cargo fmt`, all-target Clippy and diff checks passed. Unified lifecycle population, LOO-406 integration and configured demo remain; no live migration or landing.

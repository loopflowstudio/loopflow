# One Task, up to one PR

LOO-418 · reviewed decisions; first implementation milestone · 2026-10-07

## Decision and intended experience

Jack decided on October 7 that a Task delivers zero or one PR, never a serial
chain. Follow-through is a follow-up Task: file it after landing, then complete
the landed Task. Dependent implementation starts in another stacked Task before
its parent merges. A recommendation can finish without a PR.

Jack Heart confirmed the completion rule during design review on October 7:
`PR merges → file follow-ups or record “none needed” → Task completes`.
Jack then clarified that no Task at `end` may remain incomplete. Restore
`lf task complete ISSUE` as an alias for `lf task move ISSUE end`, sharing one
transition and its checks. Jack subsequently requested continued advancement.
The first implementation milestone is Flow launch/status correctness and the
completion alias. The remaining lifecycle mechanisms retain their stated review
status; this milestone neither completes LOO-418 nor authorizes landing.

The experience: a Task has one outcome, an optional PR link, and one Workflow
position. After delivery, its page says either **Done · Follow-up LOO-…**, or
**Merged · Follow-through pending**, with the reason and next action. It never
says “PR 2,” retains an unexplained open state, or waits for production evidence
that belongs to another Task. Research reaches `end` without a phantom PR slot.

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

The first implementation demonstration is the sibling-checkout Flow launch
repair (slice 1 below). It is useful independently, but is not completion of
LOO-418's larger contract.

## Inventory and findings

Inspected base: `626789dcd0382c6754ba3b7c61ea2448b57658ce`.

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

`bin/lf.rs` admits the Process at the caller's cwd before `dispatch` resolves
`--task` and enters `binding.cwd`. `run_for_task` spawns its child without setting
the target cwd. The Flow's recorded driver consequently keeps the caller's cwd.
Both the Started trigger and `store/sqlite/task_work.rs::flows_of_task` depend on
that location. `flow_inventory::entry_in` separately duplicates a cwd join.
An explicitly bound Session may eventually make part of the work visible, which
does not repair a mechanical Flow or its driver membership.

A disposable SQLite probe executed the actual released Started trigger with one
Task and one recorded Flow. Caller cwd reproduced “Started requires recorded Task
work”; changing only that row's cwd to the Task checkout allowed Started. This
proves the predicate failure and, with the dispatch ordering, a concrete defective
path. It is not a replay of every October 7 launch; path aliases and direct
`--task` launches need public CLI regression coverage too.

### Provider support

Linear's official generated SDK defines `IssueCreateInput.id` (UUID v4),
`dueDate` (`TimelessDate`) and `IssueRelationCreateInput`, including related issues.
Use a generated UUID v4 persisted before mutation, not a hash disguised as UUID v4.
The client in this checkout only sends basic issue creation fields and does not
project due dates or issue relations. Extend that existing client and PM model.
Schema support is verified; live writes and retry semantics are not yet proven.

Sources inspected October 7: [Linear SDK schema](https://github.com/linear/linear/blob/master/packages/sdk/src/_generated_documents.ts),
[issue relations](https://linear.app/docs/issue-relations),
[due dates](https://linear.app/docs/due-dates).

## Decisions and proposed mechanisms

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
recovery depends on an installed operator schedule. Polling limits and the exact
filing interface below remain proposals.

Keep `lf land` and `lf arm` as prepare/request/return operations. Add an explicit
`--wait` to land for callers that own a blocking Flow. It uses the existing finite
landing observation operation, releases its lock between checks, and never runs
an agent inside reconciliation. Poll every 15 seconds for at most 30 minutes;
interruption or timeout returns held, leaving merge intent and evidence intact.
These are proposed initial operational limits, not measured performance claims.
CI repair remains the existing CI watcher's responsibility.

Change `ship` to `gate → cmd: land --wait → follow-through`. The `follow-through`
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

Repurpose `lf task follow-up ISSUE`: it now files or links actual Tasks, or
records that none is needed. Remove the current `--outcome/--evidence/--check-at/
--clear` contract. Proposed forms are `--title … --notes … --due YYYY-MM-DD`,
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

**Proposed authorization boundary:** filing creates an obligation to return to,
not permission to execute arbitrary future work. The pass surfaces a due Task
for selection unless its brief already authorizes a concrete unattended check.
An authorized check uses ordinary Task execution and the existing liveness
inspection. Missing credentials, installation, date or provider evidence leaves
the child visibly blocked/unknown. Time passing is never proof of success.

Use an already installed Wave schedule or its ongoing conversation. Do not add a
resident, a per-follow-up scheduler, or enable a schedule as a side effect of
filing. If no schedule is installed, report “next Wave pass; no automatic check
scheduled.” Exact unattended “tomorrow at 09:00” service is excluded; Jack's
review must confirm whether this return-to-Wave behavior is sufficient.

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

Delete together:

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
its own Task and child-specific design. None of these slices is implemented yet.

**Current milestone:** slice 1 and the `task complete` alias from slice 3,
through focused verification and demo. The alias shares today's end operation;
the stronger post-merge follow-through checks arrive with slice 3. Preserve the
remaining scope below, and do not present the milestone as the complete Task.
Jack has not settled the remaining product choices under Remaining review
boundary; this milestone does not require them.

1. **First demonstration — truthful Flow launch.** Correct admission/child cwd and
   shared Flow association. Add a held mechanical Flow in
   `task_flow_launch_tests` launched from a sibling checkout; assert target
   Started, target inventory and status while still running, plus retained
   caller attribution. Cover direct `--task`, plain checkout invocation, alias
   paths and a read-only command that must not set Started. Focused command:
   `cargo test -p loopflow --test task_flow_launch_tests` using the isolated
   fixture conventions in TESTING.md. Show the real CLI result before moving on.
2. **Remove serial authority and placeholder coupling.** Migrate placement,
   optional current PR and read-only history; delete rotation and cut CLI/Swift
   consumers over in the same slice. Preserve stack/restore/file operations.
   Prove research with committed findings can end without a PR and retains its
   files, and an open published PR cannot masquerade as completed research.
3. **Finish delivery through linked Tasks.** Replace keep-open obligations,
   implement durable filing/retry and due-date projection, update land waiting,
   ship/finish Flows, operator guidance and shared pending/done presentation.
   Restore `task complete` as the end-move alias, used by follow-through and the
   Task operator; keep the existing completion operation as the single owner.
   A stopped post-land Flow must be finishable without rerunning delivery.
4. **Carry design into dependent work.** Add explicit checkout handoff and
   update decomposition guidance. Exercise two children with distinct designs,
   repeated placement, parent code/scratch changes, and parent squash merge.

Gate runs the changed-aware headless plan once:

```sh
uv run python scripts/test.py --list
uv run python scripts/test.py
```

Its Rust/Swift/format/Clippy plan must include the affected scenarios below;
add missing coverage to existing suites rather than relying on a zero-test
filter. Use the released-frontier installation harness for populated upgrade:
`uv run python scripts/test_task_installation.py`. It requires capable Docker CI,
never an experiment pointed at an installed Machine. Rust command-tree changes
also run `cargo test -p loopflow --lib engine::flow_graph::tests`.

| Observable result | Headless proof owner |
| --- | --- |
| One merged Task becomes end/done only after its zero-or-more follow-ups are durably resolved; CLI, work monitor and Desktop agree on PR, state and links | Extend `task_flow_launch_tests`, `land_tests`, DTO fixtures and `TaskFlowProofTests`/`RegistryQueryTests` with the same lifecycle population |
| `task complete`, `task move end` and automatic arrival produce the same end/done result; refusal leaves position unchanged; completion within follow-through followed by driver arrival or retry produces one transition | Task launch integration, Task authority tests and command-tree coverage; operator prompt scenarios include merged but unfinished Tasks |
| Real Git fixture plus simulated GitHub merge and Linear mutation crosses store, public CLI JSON, monitor projection and Swift decode/view; no live provider or display is required | New lifecycle case in existing Task launch suite; serialize its resulting wire fixture for both Rust `dto_fixtures` and Swift `DTOFixtureTests`/headless Task view assertions |
| Lost creation response, crash before local receipt, simultaneous finishing callers, issue edited/moved and chapter rotated all reuse one child; provider failure leaves explicit pending state | PM/Linear tests, `task_pr_authority_tests` and Task launch integration; assert issue population and Task state, not mock calls |
| Bare land, waited land and out-of-band merge all converge; wait timeout/interruption does not clear intent, replay gate or complete early; reconciliation between merge and filing retains the checkout for the next step | `land_tests`, `pr_landing` tests and a public CLI held-Flow case |
| Research reaches done with `pr: null`; retained drafts/commits and Sessions stay reachable; no hidden Working PR is created | `task_initialization_tests`, `task_diff_tests`, Task launch and shared DTO/view fixtures |
| Child publishes before parent merge, accepts parent updates and survives squash with one PR and its own design; changed handoff input never overwrites child edits | Existing `sync_tests` stack scenarios plus `task_initialization_tests` |
| Due tomorrow is absent from today's due list, present on the next due Wave pass, and never auto-completes; unscheduled coverage is stated honestly | PM projection/operator prompt scenarios with fixed dates; Desktop/CLI use the same due date |
| Populated zero/one/multi-PR and old remaining-work records retain IDs, active code, pending obligations, history and provider writeback under upgrade | `store::migrations` plus disposable installation harness |

Fixtures prove the shared contract, not sustained use. Jack's demo review judges
whether the Workflow and Task page make sense. Product's chapter KRs still need
three working days of primary Desktop use and three two-hour sessions without
crash or access loss; this design claims neither. No new performance metric is
needed for these bounded reads and existing UI surfaces. Gate should verify
that ordinary Task reads add no per-follow-up subprocess or network call.

## Remaining review boundary

Jack's zero-or-one PR and post-merge follow-through check are accepted: file
follow-ups or record none needed before completing every merged Task. Jack also
accepted the landing Flow/operator recovery approach and clarified that end is
completion, with `task complete` an alias for moving to end. Remaining proposals
include polling limits, filing interface, removing `-c`, explicit design handoff,
and due follow-ups returning to the owning Wave without enabling a schedule.
Live migration, closing LOO-385, automatic filing of unrelated improvements,
publishing/landing this branch and claimed production acceptance are excluded.

Check (2026-10-07): `uv run --no-project python` disposable SQLite trigger probe passed (caller cwd rejected; target cwd accepted); source/API inspection only, implementation suites deferred to their slices and gate.

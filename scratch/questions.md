# LOO-298 review agenda

2026-09-26. Interactive review amends Jack's earlier model. The current
participant's name is unresolved. Approval covers the changes recorded below;
implementation assumptions are distinguished from explicit confirmation.

1. **Naming — confirmed.** Flow = template, Invocation = execution; use
   “Flow invocation” in full. No CLI-wide rename is proposed.
2. **Taskless Flows — resolved in interactive review.** The current participant
   (name unresolved) explicitly requires Flows to work without a Task.
   Invocation Task is nullable; taskless and Task-owned execution use the same
   SQLite records and driver. Existing taskless invocations import with their
   captures, progress and review Sessions intact, without synthetic Tasks.
   `Task ⇒ Wave` remains; `invocation ⇒ Task` is removed. The existing nullable
   parent equality still constrains binding an invocation Run; whether bind may
   attach a taskless invocation's individual Run to a Task remains for review.
3. **Historical missingness.** Many old Runs have no capture proving whether
   they were independent. Proposed `membership_known` preserves Unknown as a
   stored fact; null invocation alone is not labeled Independent for those rows.
   No runtime fallback to manifests/subjects is retained.
4. **Identity on replacement — confirmed.** Session owns Runs and a current
   Run. Stable Session ID, title and feedback survive Run replacement; prior
   Runs remain history. `runs.session_id` supplies the history and
   `sessions.current_run_id` selects a member. Validate that relationship and
   change the pointer with the exact pending attempt atomically.
   Cross-Run bind scope was not explicitly decided: the working assumption is
   current Run ancestry, inherited by replacement, with no bulk history rewrite.
5. **Operational consequences — amended by Jack below.** Every launch requires
   a writable local Run store. Bind fills missing ancestry once; no Run can move
   or clear its Task. Started is any Task Run, with a set-once timestamp at first
   assignment. Nullable invocation equality and separate retirement evidence
   still apply. The final dated decisions below supersede reversible binding.

Implementation assumptions: preserve manifest/context/terminal evidence and
provider-native history; “every sidecar goes” concerns mutable product and
attachment state. Real-Home conversion requires a separate maintenance invocation
after quiescing exact old writers, with backup/rehearsal first. The present slice
neither inventories private Home contents nor changes them.

Integration finding: this checkout predates several LOO-291 features and already
dropped the former `runs` table. Reconcile actual source at implementation time;
do not reapply the interrupted `d0-docs-partial.patch` over the completed doc edits.

## First implementation cut (2026-09-26)

The selected implement step follows the supplied completed-review feedback.
The four Task step projection columns (`flow`, `step`, `node_id`, `human`)
can be removed without introducing a second owner. Root `step_index` and
`iteration` cannot yet go: historical flat review records still need them.
Their conversion stays with the complete invocation migration.

Review discovery now decodes each current Task position and selects the captured
human policy, including nested XOR paths. This is a bounded intermediate reader
over current Task positions, not the final indexed Session inventory and not a
latency acceptance claim. A malformed autonomous capture is now surfaced by
review discovery rather than skipped by the former SQL `human=1` filter.

Verification observation: `scripts/resource_envelope.py` and its safe `--recover`
pass both report the active `main-view-task` checkout at 14.5 GiB against its
12 GiB budget, while the disk has 100 GiB free. Recovery correctly leaves the
active checkout alone. TESTING.md stops product tests on unresolved pressure;
this worker does not delete another contribution's active build output. Static
checks continue. Behavioral tests remain required once that resource condition
clears; no real Home import or installed binary promotion is authorized by this
local proof.


## Invocation lifetime cut (2026-09-26)

Executive sequencing choice: retain Task invocations before replacing the other
owners. The existing completion/restart paths deleted their captured execution,
which cannot supply the approved historical Run/Session ancestry. The new table
is the sole Task execution owner; the remaining ordinary file path is existing
unfinished cutover scope, not a newly selected dual-store design. No intermediate
publication is authorized by this slice.

The forward migration adds a Task FK and requires each captured invocation ID
to be unique and nonempty. Old singleton storage did not enforce either rule.
A conflicting historical Home must stop and preserve its transaction input for
explicit mapping; the migration does not rewrite IDs, drop captures or null Task
ancestry. This pass inspected no real Home, so conflict frequency is unknown.

Preflight and safe recovery now observe `main-view-task` at 15.3 GiB / 12 GiB
(98.9 GiB free). Product tests remain blocked by TESTING.md. The outstanding
commands also include `retaining_invocations_preserves_populated_execution_and_review_bytes`
and `stale_human_decisions_cannot_target_a_replacement_invocation`; run the
migration proofs again after materialization in a disposable source copy.

## Task review Session owner draft (2026-09-26)

Implemented the first used Session/Run transaction path for Task reviews. This
is a narrower internal implementation boundary than the requested all-caller
cutover; the full design and all acceptance obligations remain unchanged. No
intermediate publication or Home activation is selected. Ask, interactive and
taskless Flow owners still need conversion together with their readers and DTOs.

New preservation boundary found during source review: the child can publish its
immutable manifest and then be interrupted before marking its reserved SQL Run
published. Retrying retains the reserved Run ID, but today's artifact publisher
rejects an existing manifest/staging directory. Exact publication recovery must
settle that boundary without starting duplicate providers or replacing immutable
input. The draft neither deletes those bytes nor treats missing publication as
proof of provider death. This is an unfinished recovery obligation, not a reason
to invent a new Run or mark the implementation accepted.

The SQL draft preserves unmapped Task review inputs under historical column
names for offline import; current writers never update them. The full importer
must classify those inputs and remove the conversion-only columns before the
final installed cutover. The draft by itself does not establish lossless import
of legacy flat captures, completed conversations or filesystem-owned Sessions.

Latest preflight still reports active `main-view-task` 15.3/12 GiB, 98.7 GiB
free. Product tests remain unexecuted; do not reinterpret all-target Clippy as
behavioral evidence. No new decision from the participant is needed to continue
within the amended model.

## Rebase onto main 4cd64be3d (2026-09-26)

The owned rebase completed locally. Documentation retains the approved model
and main's active-Run watch protocol, Ask tables, and required Session Run ID.
Later commits also conflicted in Task review launch/controller code. Task
reviews reserve their Run in SQLite and publish that exact ID with captured
Flow membership; Ask and standalone Flow preparation retain main's file path.
The existing rename command keeps generated suggestions and routes Task review
names to the Session row. Duplicate rename declarations were removed.

Verification is blocked before behavioral execution. Both resource preflight
and safe recovery reported the active `jack-heart/main-view-task` build at
15.3 GiB against a 12 GiB budget; this checkout uses 331.5 MiB and free disk is
97.3 GiB. Recovery did not remove active build output. No compiled or passing
behavioral result is claimed for the reconciled Rust. The waiting parent must
verify before pushing. The focused reconciliation command is:

```sh
cargo test -p loopflow --lib controller::task::planning_tests::claimed_autonomous_boundary_settles_once_at_the_human_node
```

The resolved files have no conflict markers. Whole-branch whitespace still
reports the previously recorded draft dependency header and copied historical
patch; neither was changed during resolution. No push was performed.

Post-rebase review compiled the reconciled Rust with all-target Clippy after
removing an unused preparation parameter. Formatting, migration validation and
HTML consistency pass; architecture retains the `wave_chapters` gap. Behavioral
execution remains blocked by the resource receipt above. The restored SQL-backed
Session naming/membership regression is compiled, not executed; details and the
current disposition are in [the slice review](data-model-slice-review.md).

The supplied Linear comment also adds two delivery reports to implementation
scope: publication of PR #1296 missing from its Task PR row, and support for
stacking an already-created Task. These were not reproduced or repaired during
review. Investigate through the existing delivery owners; do not infer that a
successful remote publication proves its Task row was updated.

## Recovery and historical lookup cut (2026-09-26)

Executive choice within the amended model: historical Run selectors return an
explicit message naming their retained Session and current Run. They do not
silently target a replacement. Stable history remains in SQL; general history
inspection and all conversation kinds still need the planned owner/DTO cutover.

Observation: publication previously constructed a CaptureHandle before claiming
SQL. Rejection could therefore settle the Run as failed through Drop. Publication
now reconciles immutable artifacts, claims SQL, and only then constructs capture.
Rebuilding different prompt/runtime/parent inputs is a named conflict, not license
to rewrite the original preparation. Complete staged files retry; conflicting or
partial file bytes remain preserved for explicit recovery.

Observation: the published flag does not establish whether a provider started.
Open now leaves a published attempt unresolved without native history or a valid
terminal outcome. The remaining Run-owned process lifecycle must recover that
case using exact evidence. Missing receipts alone cannot authorize replacement.
This is not a claim of complete automatic preparation/launch recovery.

Resource preflight and safe recovery again fail on active `main-view-task` at
15.3/12 GiB with 97.3 GiB free; no behavioral test or materialized rehearsal ran.
Ordinary implementation continued. The complete eight-part acceptance and the
supplied delivery/stacking reports remain unfinished; no new Ask is needed.

## Started correction (2026-09-26)

Jack's final correction in Linear comment
`ba9ec6d3-2bd4-463f-91e0-2bb7045fe72f` supersedes the two intervening
Started steers: Started is one fact, any Run with `task = X`. Keep one indexed
existence reader. No worker-claim bit or worktree test defines Started. The
architecture-reference sentence and its separate Chapter-retirement caveat
remain the accepted contract. Remove `record_task_start` and the Started event
only when the derived query serves every reader; retained historical evidence
must still protect retirement. The current partial query includes events,
worker generations and published review Runs; its passing test will establish
that transitional behavior, not the final any-Run contract.

The current review executes the accumulated proof debt before another owner
conversion, following comment `1d8e2427-2338-466e-92c7-f2bc27526712`. Fresh
resource preflight passes (107.3 GiB free, main-view-task 546.8 MiB). The first
counterexample was a corrupt-neighbor fixture inserting null required Task
planning fields, failing before its intended corrupt capture. The fixture now
copies planning fields from the valid Task; production's required-title
contract is unchanged. Actual rerun results belong in the slice review.

## Write-once bind (2026-09-26)

Jack selected null-to-Task binding in comment
`7f5c129f-d90f-489d-ac88-f68e13fb2788`. No Task move or clear, no unbind or
`--repository` form. Parentless Runs may take a Wave or Task; Wave-only Runs
may take a Task in that Wave. Every bind surface confirms the exact permanent
target once. This supersedes reversible-binding proposals in the copied
LOO-291 evidence and the earlier active docs. Started remains any attributed
Run and becomes monotonic; usage/history never move between Tasks. Nullable
invocation Task equality remains; cross-Wave Task moves remain a distinct
operation. The current-Run-only scope assumption does not authorize rewriting
prior Runs. Current implementation has no general bind operation; the next
writer cut must enforce these rules transactionally and prove racing binds.

## Started timestamp: final correction (2026-09-26)

Jack's comment `2ecb585f-fc78-4b75-a356-aa3cc678b85c` supersedes the two
MIN(created_at) steers. Definition remains any Run with this Task. The shared
Run writer sets `tasks.started_at` once at the first assignment's time (launch
or bind), only when null and in the same transaction. Later assignments never
change it, including older Runs. Validator checks presence iff a Run exists,
not timestamp equality. First bind uses bind time. Imported Tasks with Runs
receive a timestamp, others remain null; historical timestamp provenance must
be explicit in the import report rather than presented as observed launch time.
Sidebar/roadmap read one column reader; remove the event writer after complete
reader conversion. This is an accepted next implementation requirement, not a
claim that the existing review-only Run writer already stores the column.

## First-assignment storage boundary (2026-09-26)

Executive sequencing: implement the accepted write-once assignment and Started
invariants at the existing shared Run table before extending its remaining
callers. SQLite triggers own the set-once timestamp, so constructor, bind and
offline import cannot disagree or race it. No bind UI or new confirmation
transport is selected here. Current-Run-only scope and exact confirmed-target
fencing remain the subsequent bind operation's obligations.

Historical SQL Runs do not establish when their Task first received them. The
forward draft uses conversion time, explicitly inferred, for those Tasks; it
never claims MIN(created_at) or an observed launch time. The offline importer
must identify this schema boundary in its report and preserve a present
timestamp when importing more Runs. Historical Started events without Run rows
are retained separately until filesystem import; no successful Run is invented.

The remaining launch paths still write Started events. Keep those readers and
writers until the general Run conversion, while counting reservations from the
new column now. This intermediate scope is not a column-only reader acceptance
claim and cannot be published independently.

## Run attempts at one position (2026-09-26)

Jack's comment `a98697b2-240c-4b88-8c94-4af7209068b8` requires 0..n Runs
per `(invocation, node, iteration tuple)` with one current attempt, for both
headless and human steps. A successful current attempt alone advances; prior
failed/interrupted attempts remain history. Step usage/duration aggregates all
attempts, while a running status line uses the current one. Public DTOs and
node/Session detail must expose that multiplicity. The two kickoff IDs
`run_d4b210c9` (stalled/interrupted) and `run_3776027a` are supplied evidence of
one position with two attempts, not complete IDs guessed for import.

Executive sequencing: establish the shared Run/current-attempt boundary before
moving the ordinary Flow file owner. The current slice adds Task headless Run
recording through the same constructor as reviews, ordered position history and
one pointer-selection helper. It does not complete the previously requested
common Task/taskless driver. Existing historical rows retain unknown location;
no cursor-based guess is made during schema migration. The offline import must
map exact attempts before activating readers over real Homes. No intermediate
publication or live Home mutation is selected.

## Concurrent first-Home read observation (2026-09-26)

On the disposable canonical attempt-history binary, `usage --json` failed with
`no such table: run_events` while `doctor --json` initialized the same empty Home
concurrently. Doctor succeeded and reported `0.12.23.001_release`; usage then
passed. Usage on a separate pristine Home also passed. All commands cleared
inherited LF authority and used private paths beneath `.lf/tmp/loo298-attempts/`.
No installed store was modified.

Source observation: `WorkCatalog::load_at` checks file existence, then
`SqliteStore::open_run_ledger_read_only` validates `run_events`. Schema creation
opens the SQLite file before migrations finish. This supports an initialization
publication race, not loss of a table in the new draft. The concurrent failing
log is retained; the sequential passes do not repair or disprove it. The complete
reader/cutover work must account for a newly visible but uninitialized store.
Do not add a missing-table-as-empty fallback that could conceal failed migration.

## Session cutover required next (2026-09-26)

Jack Heart's comment `50cf5b12-8a17-4390-a6a1-be69a50cbb7f` requires the next
implement pass to switch one complete reader and delete its old authority.
The selected path is Session list/lookup, including Ask, interactive and taskless
Flow recording/import in that same pass. Remove four-source concatenation, old
ID dispatch, derived Session ancestry, name sidecars and title copying together.
Measure non-test additions/removals against the merge-base and report
`human_session.rs`'s non-test line count; above 2,351 is not a cutover. A lower
count still needs reachable-path deletion proof. This is an explicit direction,
not an open product question or authorization for unverified real-Home activation.

The attempt-history review reproduced and repaired a cursor-decoder disagreement
that broke historical review replacement. Its 49 source-suite passes have the
boundaries recorded in [the review](data-model-slice-review.md). That bounded
repair does not satisfy the requested Session cutover. The review names exact
remaining dependencies; another owner-only foundation is not the next action.

## Cut 1: interactive Sessions as rows (2026-09-27)

Executive choices, recorded in [the cut report](cutover/cut-1-interactive.md):
interactive Session ids are `session_<uuid>`, distinct from Run ids; Sessions
list from launch (`waiting`) instead of from first provider history; `state` is
derived, not stored; only independent `tui` launches create a Session, so IDE
handoff and headless Runs still have no rows; an interactive launch fails when
the store cannot be written. Interactive Sessions that exist only as Run
directories on old Homes stop listing until the import cut.

## Cut 2: Ask Sessions as rows (2026-09-27)

Executive choices, recorded in [the cut report](cutover/cut-2-ask.md). The
keyed retry identity is the Session id (`ask_once_<hash>`), not a separate
column. A completed Ask stays as a closed row holding its answer, keyed or not.
`sessions.request` holds the question; `runs.caller_run_id` holds the asking
Run as causality only. Ask `detail` is now the selected Skill. A headless
caller has no Run row yet, so its Ask inherits Work from the caller's manifest
until the headless cut.

Reversal of Cut 1, decided on Jack's behalf under the "prefer flexible, not
rigid" doctrine: a launch never fails because the store cannot be written. The
Session and Run wait in `unrecorded-session.json` beside the Run and are stored
by the next `lf` operation that resolves that Run. This is a deliberate
exception to "ordinary reads never create rows": it replays one reservation the
launch itself wrote, and never reads old Homes' files. An Ask reserved with no
reachable store records no Work. Jack has not reviewed either consequence.

Observation: the known first-Home initialization race reproduced once here
(`no such table: task_flow_positions`) when a waiting Ask caller opened a fresh
store while another command initialized it. The caller no longer opens a store
while its reservation is unrecorded. The race itself is not repaired.

## Cut 3: saved Flow reviews as rows; no sidecar (2026-09-27)

Executive choices, recorded in [the cut report](cutover/cut-3-flow-review.md).
Jack has not reviewed any of them.

**No sidecar, decided on Jack's behalf.** `unrecorded-session.json` and every
hook that replayed it are deleted. A launch never fails because the store cannot
be written: the launch proceeds, prints one warning line naming the store error,
and that Session is not recorded. Nothing records it later, so it never lists and
its Run id resolves to no Session. This reverses Cut 2's deferred recording and
follows Jack's standing "no sidecars" decision.

**An Ask is refused when the store cannot be written.** This narrows the rule
above. An Ask's only product is an answer, and the answer returns through its
row. Launching a conversation whose answer cannot return would strand the person
answering and the Run asking, so `lf ask` reports the store error and opens
nothing. Cut 2's test expected the opposite; it now asserts this.

**A saved Flow's review carries its Work's Wave, not its Task.** The invocation
of `lf --task X flow <name>` has no Task: one Task has one current invocation,
and that one is the Task's managed Flow. Run and invocation Tasks must agree, so
the review's Run stores the declared Work's Wave and a null Task. The Session
DTO's `work` is therefore the Wave where it was the Task. The launched review
still receives `--task X` for its prompt context.

**The saved Flow's position file is still the driver's owner.** The
`flow_invocations` row of a saved Flow is written when the Flow first waits at a
review and rewritten at each later review; it holds the capture and the cursor
of that review, and is marked completed when the Flow finishes. Between reviews
`flows/<id>/position.json` alone knows the cursor, the active headless boundary
and the failure. A saved Flow that never reaches a review has no row. This is
two owners of one cursor until the common driver cut.

Smaller consequences: a plain `lf ask` whose launcher fails now leaves its
stored Ask open (it was stored nothing); `lf session open` can still launch it.
Task review Runs record their provider and model when they publish. Task review
Sessions now report `terminal_ids`. Readiness no longer takes the launch lock;
the store fences it on the current Run. Saved Flow reviews that exist only in
`position.json` on old Homes stop listing until the import cut, and their
`session-name.json` titles are not read.

## Cut E: started, bind, Task Runs (2026-09-27)

Executive choices, recorded in [the cut report](cutover/cut-e-bind.md).
Jack has not reviewed any of them.

**Bind asks for no confirmation.** The design said the command states its exact
target and confirms once. `lf session bind` writes at once and reports the
Session's Work. A prompt would stop an agent or a script that calls it.

**Bind refuses a Session in which any Run has a Task, the same Task included.**
Jack's rule was that binding a Run that already has a Task is refused and names
it. A Session whose Runs disagree cannot arise from a launch or a replacement,
so the refusal covers the whole Session instead of filling around the bound Run.

**Bind refuses a Flow review.** A review's Run names its invocation, and a Run
and its invocation must agree on the Task. The refusal says so instead of
surfacing the trigger's message.

**`lf runs --task` lists rows only.** It has no seven-day window and no cap.
Runs with no row no longer list under `--task`: those begun by the Wave runner,
PR landing, `lf ops` and child agents. They listed before when their recorded
subjects matched. `--wave`, `--project`, `lf usage` and `lf activity` still
scan Run records, so the two readers can disagree until those launches store
rows.

**A direct launch stores a row only when it names Work.** Every launch the old
Started writer covered now stores a Run, which is what let that writer go. A
headless launch with no Task and no Wave still has no row.

**A saved Flow's headless step launched with `--task` names the Task and no
invocation.** Cut 3 gave that Flow's review Runs the invocation and the Wave,
with no Task, because the invocation has none. The headless step keeps starting
the Task as it did before, at the cost of a row that omits its Flow membership.
The two kinds of Run in one Flow now carry different parents.

**A launch is no longer refused for an unready Task or a Chapter that is not
current.** Those checks lived in the deleted Started writer. Nothing replaced
them.

**`lf session list --task` was not added.** The design's demo names it; this
cut's brief did not.

## Cut D: one-time import of old Session files (2026-09-27)

Executive choices, recorded in [the cut report](cutover/cut-d-import.md).
Jack has not reviewed any of them.

**The command is `lf session import`, with `--dry-run`.** It imports the Home it
runs in. The earlier sketch's `--from <home>` was dropped: every other command
selects its Home the same way, and a copy is imported by running there.

**An imported interactive Session keeps its Run id as its Session id.** Old
Homes named these Sessions by their Run, so links and habits still resolve, and
the id makes a second import a no-op. New launches get a separate Session id.

**A terminal Run of a Flow step with no stored Session is reported, not
imported.** Old Homes listed these as interactive Sessions. They were attempts
of a review, and storing them as independent conversations would misstate
that. On the copy of the installed Home this was six Runs. They no longer list.

**A Run whose manifest names an unregistered Task or Wave is reported, not
imported as an orphan.** Importing it without its Work would drop the
attribution silently. None occurred on the copy.

**`started_at` for a Task first named by an imported Run is the time of the
import.** The report lists those Tasks so the inference is visible. Seven on
the copy.

**A completed Ask or review closes at its file's modification time.** The old
files recorded no completion time.

**A name in `session-name.json` replaces a generated title on a stored row and
never a human one.** A generated title suggested after the import would be
replaced by the file's if the import ran again.

## Cut F: every Run is a row (2026-09-27)

Executive choices, recorded in [the cut report](cutover/cut-f-runs.md).
Jack has not reviewed any of them.

**A Flow's step Run may name a Task while its invocation names none.** This
reverses Cut E's choice that such a Run names the Task and no invocation. A
Flow launched with `--task` owns an invocation with no Task, because a Task's
one current invocation is its managed Flow. The draft
`name_tasks_on_flow_step_runs` relaxes the trigger: an invocation that names a
Task still refuses a different one. The stricter reading, Wave only, made
`lf --task X flow …` stop listing under the Task and stop starting it.

**A saved Flow's review Runs still carry the Wave and no Task.** Cut 3's choice
stands, so a Flow's step Runs and its review Runs share the invocation and can
differ in Task. Bind still refuses a review.

**An imported Run of a Flow step names its Task and Wave and no invocation.**
The constructor checks a Run's position against the invocation's cursor, and
an old Flow's cursor has moved on. The manifest keeps the membership.

**A Run that names no Work inherits its caller's Task and Wave.** The caller is
the Run in `LF_RUN_ID`. A caller with no row gives nothing.

**A Run the store could not take is never recorded later.** Same rule as Cut
3's Sessions. Its record stays on disk and `lf runs <id>` still reads it.

**Listings read the row's end.** `terminal.json` is still written first and is
still what Flow recovery reads. A Run whose end write failed lists as
unterminated.

**`RunSnapshot.subjects` comes from the row.** The field keeps its shape. A
bound or inherited Run now lists its Work, and the selectors are current names:
a renamed Project lists under its new slug.

**`lf runs --wave`, `--project`, `lf usage` and `lf activity` list only Runs
with rows.** An older Home lists nothing there until `lf session import` runs.

**A claimed worker still counts as started.** `task_started` and
`chapter_task_evidence` dropped their Started-event clauses and kept
`worker_generation>0`. The Started event is still written because Wave chat
renders it.

**A test that failed before this cut was rewritten.**
`parked_human_boundary_reports_blockers_without_provider_preflight` failed at
`7714e218d`. It now asserts the review's Session and unpublished Run.

## Cut G: deletion (2026-09-27)

Executive choices, recorded in [the cut report](cutover/cut-g-deletion.md) and
[the research](cutover/cut-g-deletion-research.md). Jack has not reviewed any
of them.

**The saved Flow's position file stays the cursor owner.** The brief asked for
the invocation row to own it if reachable. It was not. Four decisions come
first: which of the Flow's launch facts become columns; whether the step token
becomes the position version, which changes stored review Session ids; how
`lf flow decide` and `lf flow route` fence a write with no Task; and whether a
saved Flow may still run when the store cannot be written.

**A Flow step's record path is derived, not stored.** `recover` finds the
Run's record under the Home of the process recovering it.

**Two import-only columns were left in place.** Nothing reads
`historical_session_run_id` or `historical_ready_summary`. Dropping them
deletes old evidence, so it waits for Jack.

**A flaky test was left alone.**
`pm_read_linear_oauth_sqlite_contention_has_bounded_failure_and_recovers`
failed once in the suite and once in three runs alone. It is outside this
Task.

## Task ↔ invocation (2026-09-27)

Jack, in the interactive concept review at `e7f3e22fd`: "you should be able to
run multiple flows at once on a task. but only one should be THE flow invocation
for a task." A Task points at its one managed invocation
(`tasks.current_invocation_id`, mirroring `sessions.current_run_id`); the
partial unique index on `flow_invocations(task_id)` goes. Every Flow launched
with `--task X` names X on its invocation and its Runs, including reviews, but
`lf --task X flow foo` never becomes THE invocation; only `lf task run X
[--flow foo]` moves the pointer. The
Run validator returns to strict nullable equality with its invocation. This
supersedes Cut 3's "review carries the Wave and no Task", Cut F's relaxed
trigger, and Cut E's blanket refusal to bind a Flow review. The saved Flow
driver and the Task driver become one executor over the invocation row; see
[the concept review](concept-review.md).

## Unwritable store and bind confirmation (2026-09-27)

Jack, same review: "I guess refuse every launch for now." Every launch refuses
when its Run row cannot be written; a Run without a row does not exist. This
reverses Cut 3's warn-and-proceed rule and unblocks deleting `resolve_manifest`,
the `prepared` marker and `terminal.json`-as-state.

On bind confirmation Jack said "don't really know, just make the best UX." The
review chose: the CLI prints the exact target and writes; the app owns a confirm
step; binding to the Task a Session already has is a no-op success. This
replaces Cut E's same-Task refusal and satisfies the earlier "confirm once"
decision at the surface where a mis-click is possible.

## Cut H1: Task points at its invocation (2026-09-27)

Executive choices, recorded in [the Cut H ledger](cutover/cut-h-one-flow-driver.md#2026-09-27--h1--task-points-at-its-invocation-strict-equality).
Jack has not reviewed them.

**`flow_invocations` gains no `wave_id` in H1.** The H1 draft adds the Task
pointer only; a saved Flow's invocation names its Task, and its Runs carry the
Wave. H2's `own_flow_launch` owns the launch facts (`cwd`, `message`, `model`,
and the Wave if it is one).

**A review Run naming the Task starts it.** `lf --task X flow <review-first>`
now reserves a review Run that names X, and the store's rule since Cut E is that
the first Run naming a Task sets `started_at`. `flow_tests::observing_and_
preparing_a_task_are_not_execution` asserted the opposite for a saved Flow's
review because Cut 3 gave that Run the Wave only; it now asserts the start.

**The bind line prints on stderr, on the no-op too.** `Binding <session> to
<ISSUE> (<title>). Permanent.` keeps `--json` stdout parseable.

**A stale review is "no longer waiting".** `owned_target` treats a review as the
Task's own when its Run's invocation is the Task's pointer; otherwise it must be
a waiting saved Flow, or the lookup is refused.

**Binding a taskless Flow's review stays impossible.** The bind-specific refusal
went, and the strict Run ↔ invocation trigger now refuses it with `Run and
Invocation nullable Tasks disagree`: neither the Run nor the invocation can take
a Task the other does not name, in either order. Open for Jack: should bind
assign a whole taskless invocation and every Run in it?

**`Task` in Rust does not carry the pointer.** Only the store reads
`tasks.current_invocation_id`; the struct grows when a reader needs it.

## Cut H1 review (2026-09-27)

Executive choices, recorded in [the Cut H review](cutover/cut-h-review.md).
Jack has not reviewed them.

**The draft backfills pre-H1 Task Flows.** `point_task_at_invocation` now gives
a taskless invocation whose Runs name exactly one Task that Task, on the
invocation and on its Task-less Runs (Cut F step Runs and Cut 3 Wave-only
reviews), so strict Run ↔ invocation equality holds for rows written before the
draft. An invocation whose Runs never named a Task stays taskless; one whose
Runs disagree is left alone rather than guessed. Only dev Homes that ran this
branch's drafts have such rows.

**Kept for Jack.** Whether `Binding … Permanent.` should print after the write
(a refused bind currently prints it first); whether bind may assign a whole
taskless invocation and every Run in it (today the strict trigger pair refuses
with its raw constraint text); who owns Done-when 4's "`lf task run X --flow F`
moves the pointer" — `ensure_flow_position` keeps an existing Flow and ignores
`--flow`, so restart is the only mover.

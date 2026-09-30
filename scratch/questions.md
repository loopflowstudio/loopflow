# LOO-303 merge onto LOO-298 — 2026-09-30

Jack Heart requested a merge of parent ab901f1f1, verification, a merge commit
and a non-force push. The parent model governs over the historical designs below.

- Kept command palette, exact Task links, historical Task selection, recursive
  folded templates and terminal input isolation. Template composition now rides
  the parent's Target/compiler traversal; graph references are captured numeric
  node IDs. Project.flow supplies the Wave-page template.
- Dropped the inherited Run/Session/FlowInvocation storage implementation,
  first-Run timestamp triggers, old draft chain, import/recovery helpers and
  Run-manifest reader. Exec, AgentSession and FlowSession remain the sole parent
  owners, with exactly the parent's three drafts. No child schema change.
- Dropped the old Chapter-table/default-Flow adapter and resident Wave plumbing.
  Exact Task lookup uses current Project planning and retained Task rows without
  launching work. The parent's file browser and active Session stream are retained.
- The old Session-owns-Runs, position-attempt and runtime-child-invocation plans
  are superseded. Their dependent room/bind/attempt slices were never implemented
  here and are not recreated in this merge. Future UX must consume the parent
  AgentSession/FlowSession/Exec APIs, not revive those proposed owners.
- Historical notes below retain their original evidence limits. They are not
  instructions to import history, restore compatibility readers, or reinstate
  the discarded schemas.

Initial resource preflight/recovery found 30.7 GiB free against the 32 GiB
emergency reserve. Recovery retained recent inactive builds and uv pruning was
locked. Ask ask_783d5000e5e144c4afe5b8e8c56d6385 requested an exception for two named
inactive build caches. No approval or cache deletion is claimed. Space increased
independently; preflight subsequently passed at 46.9 GiB and again at 43.9 GiB.
Verification resumed without the proposed deletion. The separate Ask note is
another active contribution and is excluded from this merge commit.

Source review preserved the parent's file browser and active Session stream
alongside palette/history navigation. Storage, Session types and controllers
are byte-identical to ab901f1f1. The compiler retains composition in its existing
Target traversal and flattens it through one implementation; disclosure IDs
never become execution IDs. No compatibility reader or historical import was
introduced.

Verification receipts live under `.lf/tmp/workspace-navigation/merge-*`:

- `cargo build`, all-target Clippy with warnings denied, and formatting pass.
- 56 Flow compiler/graph/decision tests and 12 DTO tests pass. The status suite
  initially passed 12/13: its old no-command-record assertion conflicted with
  Exec owning every CLI command. The repaired test proves no Sessions or Started
  evidence are created by inspection, and passes against the real isolated CLI.
  The affected chapter-deletion planning regression also passes: 82 distinct
  Rust tests passed in total across these focused commands.
- SwiftPM builds LoopflowMac. All 54 selected Swift checks pass across the
  original run and the focused Flow rerun. The first native Flow check exposed
  stale numeric IDs in the merged catalog fixture. Repair preserves the nested
  XOR and final command, asserts full expansion equals the graph, and passes all
  six Flow checks, including keyboard disclosure and retained PTYs.
- Architecture passes all eight inventories (including 33/33 SQLite owners),
  Swift platform boundaries pass, and migration validation reports only the
  three parent drafts with released migrations preserved.

Native checks use fixture transport and owned PTYs; these results do not claim
installed-app acceptance, provider interaction or a live Home migration.
XcodeGen and signed ad-hoc Xcode `build-for-testing` pass (87.1 seconds),
including the app and test runners with the terminal fallback. Hosted UI tests
were not run. The corrected catalog fixture also passes Rust round-trip
verification. Final formatting, platform boundaries and child-diff whitespace
checks pass.

Delivery uses `lf commit --no-add` and plain `git push`. The inspected
`lf commit --push` implementation retries a failed push with `--force-with-lease`,
which conflicts with Jack's explicit non-force boundary. No history rewrite,
release activation or Task completion is included.

# LOO-298 review agenda

LOO-303's current design is [Workspace UX after the data model](workspace-ux-on-data-model.md).
This inherited LOO-298 agenda remains evidence, not this Task's implementation plan.
Its later `data-model-slice-review.md` reference was not copied into this checkout;
those review claims retain their original limits and have not been revalidated here.

## LOO-303 parent dependency assessment (2026-09-27)

The [source/fixture receipt](workspace-parent-contracts.md) replaces the earlier
local-only readiness assumption. Published LOO-298 `ca1be1116` has attempt
storage; local `d814eb617` adds interactive Session rows. The Task now records
PR #1296 correctly. Neither revision provides the complete all-kind Session,
Bind, Started-consumer and public attempt/history contracts. Uncommitted Ask
conversion is ongoing source evidence, not an integration revision. Keep this
Task's parent-dependent slices pending that conversion; no duplicate storage or
Swift attempt inference. No new user decision or repeated keyboard repair is
needed. This is an implementation dependency, not a claim about parent liveness.

## LOO-303 recursive keyboard counterexample (2026-09-27)

Dispatched Tab from Task details reached a hidden `GhosttyMetalView`: its
`acceptsFirstResponder` remained true after SwiftUI disabled the retained host.
The focused template cut now carries that existing enabled state into AppKit
responder eligibility. This fixes input isolation without changing host mounting,
terminal lifetime, or the deferred one-mount control-room design. Native disclosure
controls also need a keyboard target at every recursive level. The unhosted
proof drives AppKit key-view recalculation after layout changes; its initial
backward-Tab failures do not establish an installed-app defect. A shared style retains
its existing expansion owner while handling pointer and keyboard on one button.
These are observed local counterexamples, not installed acceptance.

## LOO-303 interactive review (2026-09-26)

The current participant (name unresolved) clarified that “wave flows” means
the Project's default Flow for Tasks, presented on the Wave page. The agent's
Wave-level execution interpretation is superseded. No Wave invocation inspector
or new Wave-level start controls belong in this design. All non-Task invocations
remain in overall monitoring, with no Task-like main-pane page.

The existing Session inventory and orphan predicate remain unchanged as an
implementation assumption; removing invocation pages does not itself remove
conversations or outstanding review work. Current-Run-only bind remains an
inherited assumption. The participant approved the corrected design on
2026-09-26. These assumptions remain explicit implementation defaults; no new
blocking design decision is required before the independent navigation slice.
See [review feedback](workspace-ux-review-feedback.md).

## LOO-303 kickoff assumptions (2026-09-26)

Jack's evening comment `41ea97a6-1532-4453-87b2-86fc7634aad2` governs:
light only, no teardown of LOO-291, palette/Task links/template first, then the
table-dependent room and bind. Session owns Runs; step positions have Run
attempts and one current attempt. No further decision is required for kickoff.

- An independent Task Run (including one bound from the room) sets Started but
  does not manufacture a Flow invocation. Keep the template with “Flow not
  started” until captured execution exists. This resolves an edge in the older
  “until a Run exists” wording and is an implementation assumption.
- Task URLs and palette Task entries open details; the sidebar's existing
  single-Session drill-down remains unchanged. Unqualified issue collisions use
  a repository chooser rather than selecting the first candidate.
- The shared model must supply position-level attempt order/current identity.
  Provider retry streams within one Run are separate. The inspected parent
  source does not yet expose this projection; no ordinal is inferred in Swift.
- Bind remains current-Run-only under the inherited assumption. Taskless
  invocation Runs obey nullable Task equality and may remain non-bindable
  orphans with a shared explanation; the room must keep them accessible.
- Multiple Session records can point at one shell terminal. Keep every orphan
  tile, mount that terminal once, and offer “Show here” on the other tiles.
  Never clone the provider client to satisfy the layout.
- Parent schema presence alone does not release table-dependent implementation.
  Complete independent slices while final shared APIs are being built. No
  duplicate store, live migration or PR-record repair belongs in this Task.

The remainder is preserved parent evidence. The full new design owns scope,
ordering, review questions and proof for LOO-303.

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


# LOO-298 decisions and open choices

Jack Heart · 2026-09-30. This is the live decision list; superseded proposals and
pass histories stay in Git.

## Settled

- One actual lf process is an Exec. Flow steps run ordinary skill/command
  children; no second provider executor. Definitions compile before execution.
- One started Flow is one FlowSession. Subflows and loop passes are display
  lenses, not child Sessions, claims or lifecycles.
- Captured input is a Session event. No Run, Request or replacement attempt
  object. Exec outcomes and provider outcomes remain distinct.
- Direct-command behavior governs Task steps. Task context follows the checkout;
  all agent starts use the same participant resolver. Credential failures belong
  to accounts and the shared retry/failover path.
- Attribution precedence: this command's --as, then checkout Task, then inherited
  explicit LF_AS. Task commands supply --as. Session ownership, process ancestry
  and claims never infer Work.
- Bind affects subsequent usage; earlier usage keeps its recorded owner. Do not
  invent token splits for active cumulative turns. Intelligence re-evaluation is
  a follow-up direction, not a Task filed by this pass.
- Blocked is structured output with a required reason. Its keyed Ask returns
  feedback to the same conversation; it supplies no navigation verdict.
- Child executable resolution: leading PATH lock, ordinary PATH, selected
  installation, driver fallback. LOO-334 owns the recursive lock mechanism.
- Delete historical import, old-format/selector compatibility and intermediate
  drafts. Keep current Work/links, accounts/routes and resumable Sessions.
  Migration rehearsals use copies only; no installed-Home write or promotion.
- One item per implement iteration, focused checks between items, full local
  Rust coverage before final gate. LOO-334 continues independently.
- Jack Heart authorized autonomous landing on 2026-09-30: “try to do this all
  autonomously, no need to review with me.” No demo or review wait remains.
  Land #1296 as one PR; #1358, #1359 and #1360 already landed the independent
  pieces. Exec/Chapter extraction would save only about 10% and requires manual
  cutting, so the old landing-groups proposal is superseded.
- Merge main into this branch; do not rebase. `e3a2c7e2c` merged #1360.
- Only this machine is a client. Jack reports the pinned dev Home is gone and
  its active Tasks were moved by hand to the one main Home, `~/.lf`. Do not
  recreate it or preserve history/compatibility for nonexistent clients. Three
  migrations remain; current operating state and resumable Sessions still matter.

## Still open

- Confinement currently applies to unattended execution in a Task checkout,
  shared by direct skills and Flows. Jack has not selected the wider
  checkout-only or unattended-only policy; preserve the current intersection.
- A Wave with no In Progress Project has no selected automatic creation policy.
  Report the missing plan. Whole-Flow binding is outside Session bind scope.
- `session_events` → `agent_events` and the earlier `exec_events` rename remain
  open, unapplied proposals. The latter has no surviving `run_events` table to
  rename. [Naming](naming.md) records that observation without closing the choice.
- Configured provider/Desktop acceptance and actual quiescent conversion need
  their own proof and authority. Fixture success does not close either.

Current realign authorization: reconcile the plan and Infrastructure memory,
commit, push without force and stop. Subsequent autonomous landing belongs to
the supervising session. [Remaining work](remaining-work.md) separates merge
requirements from release conversion of `~/.lf`; neither installed migration
nor promotion is part of this pass.

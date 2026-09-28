# LOO-298 parallel work

2026-09-28 · Jack authorized parallelizing as much as possible without reducing
quality. This allocation supersedes the earlier one-code-writer implementation
sequence, preserves the full accepted scope, and creates no second Task or
worktree. All contributions use Codex through installed lf.

## Ownership

- **Existing managed worker:** Exec/AgentSession/FlowSession conversion, native
  connection, public CLI, Session/Run history and import, shared store modules,
  Rust/Swift consumers, core docs and final integration. Retains the saved Flow
  claim. Do not edit the paths reserved below while their contributors run.
- **Schema performance contributor:** only
  `rust/loopflow/src/store/migrations.rs`, its tests in that file, and
  `scratch/parallel-performance.md`. Reduce repeated expected-schema construction
  without weakening actual schema, ledger, prefix or foreign-key validation.
  No installed-Home access or migration-history rewrite.
  A follow-up now retains this same file for the reproduced first-Home migration
  race. It owns `scratch/parallel-initialization.md` and an optional exact
  shared-file patch. Preserve the existing schema memo and its drift tests.
  Main retains journal/Exec admission policy; no build slot is granted.
- **Chapter contributor:** `rust/loopflow/src/pm/mod.rs`, `pm/linear.rs`,
  `ops/chapter.rs`, `ops/chapter_tests.rs`, `work/chapter.rs`, `work/project.rs`,
  `planning.rs`, `store/chapters.rs`, `store/sqlite/chapters.rs`, a uniquely named
  Chapter draft migration, `docs/waves.md`, and `scratch/parallel-h7.md`.
  Implement accepted status-based repository rotation and Project Flow behavior.
  Shared-file changes (CLI, store facade, Task/Flow integration, DTO/Swift and
  core docs) belong to the managed worker: leave exact edits as a patch/handoff,
  never silently modify those paths. Coordinate schema dependencies with the
  new execution model; retain Task identity and execution across transfer.
- **Existing-Task stacking contributor:** inspect the retained incident and
  current placement/delivery APIs; own only `scratch/parallel-stacking.md` and
  `scratch/parallel-stacking.patch`. Prepare a concrete repair and focused
  preservation proof as an unapplied patch. Shared Task/delivery code remains
  with the managed worker; this contributor applies no executable edits.
- **Incident audit contributor:** read-only source/history audit of the retained
  missing Task PR publication and cancellation/refused-start reports. Owns only
  `scratch/parallel-incidents.md` and an optional unapplied
  `scratch/parallel-incidents.patch`. No builds, executable edits, configured
  provider calls or duplicate stacking implementation. The managed worker owns
  any resulting repair and its proof.
- **Publication repair contributor:** after the incident audit, prepare the
  acknowledged-identity ordering repair as an unapplied patch only. Owns
  `scratch/parallel-publication.md` and `scratch/parallel-publication.patch`.
  Main keeps all Task/PR executable files and the build slot. No provider or
  remote Git operation is authorized by this contribution.

Contributors may inspect everything. Do not commit, stage, rebase, publish,
install, navigate the Task Flow, edit shared config or mutate configured Linear.
Preserve all supplied dirty work. A compile error in another contributor's
owned file is integration evidence, not authority to edit it.

## Verification and integration

Only one Cargo build/test batch runs at once; do not compete for the build lock
or run global formatting over another writer's files. The supervisor grants
build slots through Task comments and reads each contributor's ready note.
Read-only inspection, source edits and isolated non-building checks may overlap.
Each contributor records its exact command, input assumptions, result and
remaining integration before returning. Tests clear inherited LF/LOOPFLOW
authority and use disposable Homes per TESTING.md. Native/provider proofs remain
Codex-only. Resource checks must not clean another active contribution's build.

The managed worker checkpoints only coherent finished contributions, integrates
shared edits and runs the combined proofs. Partial passes do not satisfy the
code-complete concept review. Chapter retry must cover the partial-status
counterexample recorded in chapters.md. Performance retains the measured
empty-inventory baseline and validates deliberate schema drift rejection.

## Observed launches

- Managed execution worker: `run_8e4800ebef6045699e0be1fac668e6cb` remains at
  implement, iteration 9 of the saved feature Flow.
- Schema performance: `run_2bb88c9cf3604abaa992db98c3085143`, Codex; launch
  completed prompt preparation and began source inspection.
- H7 Chapters: `run_f328afde9d3e48bb86fef693dcd14506`, Codex; launch completed
  prompt preparation and began source inspection.
- Existing-Task stacking: `run_17b79f6272e848368b0f6f904486eb2e`, Codex;
  launched after the performance contributor finished. Its output is an
  unapplied patch and handoff, with no executable-file ownership.
- H7 preservation follow-up: `run_dd455dc0fe4947f5baa6b9e20430d7d5`, Codex;
  original H7 source ownership retained, no build slot.
- Incident audit: `run_f64d556882114e879409fd233a2007f9`, Codex;
  read-only source/history work, no source ownership or build slot.
- First-Home initialization: `run_5e039f52e291497cbb5ca8beded3756b`, Codex;
  owns migrations.rs and its ready note, no build slot.
- Publication repair patch: `run_06d28fdd07424f598ea79899c0d089fe`, Codex;
  patch-only contribution, no executable-file ownership or build slot.

Initialization contributor has returned successfully. Its source is released
to the managed worker; `parallel-initialization.md` owns the exact commands and
remaining fixture obligations. Supervisor source review checked that the read
snapshot ends before acquiring the writer, all migration decisions are repeated
under one transaction, released-draft adoption no longer nests a transaction,
and contention fixtures coordinate the waiting writer without sleeps. This is
source review, not compilation or behavior proof. Keep the actual first-Home
CLI reproduction and missing-start-event assertion in combined verification.

The supervisor strengthened `.lf/tmp/probe-first-home-execs.py` for replay:
`--lf PATH` selects the newly built candidate (default `target/debug/lf`),
copy hashes are compared before and after, and every round requires exactly
one started and completed event per succeeded Exec, matching recorded times,
zero agent Runs and no warning. Failure now exits nonzero with its receipt.
The original diagnostic script is preserved as `probe-before.py` beside the
failing first-Home receipt. Only argument parsing was checked after this harness
edit; the repaired candidate has not run through it yet.

H7 follow-up has now returned successfully. Its reserved source paths are
released to main. Read `parallel-h7-followup.md` and apply/reconcile only its new
shared patch; the original H7 sync patch was already integrated. Formatting,
populated SQL replay, eight GraphQL shape checks and patch checks passed in the
contributor; Rust behavior remains unexecuted. The build hold is released once
main integrates this last shared patch and passes resource preflight. The
publication contributor is patch-only and does not hold source or the build slot.

The incident audit returned successfully. Its source-grounded handoff is
`parallel-incidents.md`: PR #1296 association has historical recovery evidence
but no proven original cause; current publication has an acknowledged-identity
recording window. Deletion/preflight retain their own proof, while released-claim
process settlement remains explicitly deferred. Main owns the narrow publication
repair and must not present Exec causal rows as process-death proof.

These bounded Runs are children of this supervisory conversation and carry
LOO-298 attribution. They do not own the Task Flow claim. The managed worker
acknowledged the allocation in `parallel-execution.md`; its preceding Cargo
batches finished before parallel editing began.

Stacking contributor returned successfully with its two assigned artifacts.
The patch is unapplied and has syntax/patch checks only. Its parent-selection
operation and publication-base reread under the existing mutation lock are
available for main-worker integration. The blanket `parent_pr_id IS ?18` change
in the generic update still needs the independent-PR-facts review below; its
test presently expects stale publication rejection, not preservation. Review
that expectation before applying it. No new contributor owns executable files.

## Current build slot

2026-09-28 integration update: the original H7 contribution returned with its
handoff; its source-only status does not satisfy acceptance. A bounded H7
follow-up retains the same reserved source paths to repair the four review
cases below (UUID contract, missing-local-Task uncertainty, archived history,
and initial legacy Project adoption). It owns `scratch/parallel-h7-followup.md`
and a separate shared-file patch; the original handoff remains evidence.
Shared store/CLI/DTO paths remain with the managed worker. No Cargo slot is
granted to this follow-up; the managed worker keeps the serialized build slot.

The managed worker has integrated the original H7 sync hook. Three shared
Project behavior tests, four Rust Project DTO tests and thirteen Swift DTO
tests passed. The stacking patch is applied with the independent-publication
observation correction; its focused behavioral proof is still due.

No Cargo batch is running. The supervisor released the performance slot after
two focused regressions passed. The wider migration command could not compile
because H7 changed ProjectFlowPlan and added Project flow/status while shared
callers still used the former API (`parallel-schema-migrations.log`). No tests
ran in that command. Candidate build/timing and wider checks wait for shared
integration; do not repeatedly compile the same intermediate source.

Performance edits are finished and available for integration; H7 and execution
edits remain active. The next slot belongs to the managed worker once it has
integrated the shared Project API. Record its exact next command and readiness
in `parallel-execution.md` before starting; no competing Cargo or global fmt.

## Review case for Chapter classification

Source observation during supervision: the old `ops/chapter.rs::disposition`
initializes `authored=Some(false)` when this Home has no Task row. A second Home
can lack that row even though the owning Home has attributed agent work while
Linear still says backlog. That absence alone cannot prove untouched work.
The H7 contributor should preserve this as an explicit two-Home classification
case and state what positive evidence authorizes expiry. Do not silently inherit
the old default or add a new shared owner merely to satisfy the fixture. This is
a review hypothesis to distinguish against actual admission/status behavior;
no cross-Home deletion was executed.

## H7 integration review — provider contract and existing Projects

The public [Linear schema](https://github.com/linear/linear/blob/master/packages/sdk/src/schema.graphql)
was read on 2026-09-28 and retained in `.lf/tmp/h7-review-schema.graphql`.
`ProjectStatus.teamId` and `statusId` are present. However, ProjectCreateInput
documents its supplied ID as UUID v4; the in-progress deterministic successor
helper sets version bits to 8. Match the provider's advertised input contract
while preserving deterministic retry identity, and exercise that shape in the
provider fixture. No live mutation was attempted.

The accepted chapters.md also requires existing non-archived Projects to remain
usable until the first rotation. The in-progress current selector accepts only
Started and the new content parser reads flow rather than recommended. Prove
the first upgrade with an existing Project in its actual prior status and old
content, preserving its default Flow and Tasks. Do not make the first rotation
require manual recreation or silently treat a deliberately Planned successor
as current. Any one-time conversion belongs in the existing transition path;
it must not become a permanent dual-format owner or an invented approval.

Static GraphQL validation passed for all seven affected operations against that
published schema: ListInitiativeProjects, CreateProject, IssueOwnership,
ProjectOwnership, ProjectStatuses, SetProjectStatus and CanceledWorkflowStates.
Receipt `.lf/tmp/h7-schema-validation.json` records schema/source hashes and
empty error lists; `.lf/tmp/validate-h7-schema.py` reproduces it. This proves
query shapes only, not runtime UUID constraints, authorization, fresh provider
state, retry behavior or configured-Linear acceptance.

Bounded contributors have not received the later Task comments as live input
(their event streams contain only initial user input). These findings remain
the supervisor and managed worker's integration responsibility until the
contributors explicitly incorporate them in their handoff. In particular, a
handoff that merely names the legacy Project conversion gap does not close it.

Legacy fixture must also include an archived predecessor. The pre-H7 writer
calls `archive_project` on predecessor Projects without changing their status.
The shared `checked_projects_with_store` now point-reads every retained Project
missing from membership, while `ProjectNode::into_pm_project` drops archivedAt.
An archived predecessor whose provider status is still Started can therefore
reappear as a competing current Project. Preserve the retained record and Tasks,
but do not promote archived history into current planning. This is a source
counterexample to exercise, not a claim about the configured Home's contents.

## Stacking integration review — independent PR facts

`ops/task.rs` records a successful GitHub publication through the general
`update_task_pr` writer after the remote effect and Linear link. A new blanket
parent-equality rejection in that writer could therefore lose a successful
publication observation when another command just changed the parent. Prefer
making the dedicated parent operation its sole writer and preserving fresh
parent state during unrelated PR-fact updates. Parent-dependent Git/GitHub
decisions still need the existing mutation coordination and fresh target facts;
do not mistake preserving observation for authority to publish against a stale
base. Include a stale publication-observation case in the patch review, alongside
parent-change and active-worker races. This directly relates to the retained
missing-Task-PR-link incident and should not recreate it.

## First-Home Exec admission counterexample — 2026-09-28

The supervisor ran `.lf/tmp/probe-first-home-execs.py` against copied candidate
SHA-256 `5dd1c368f1ed383821ec81eb4df3cbbef981771d7e3823fdbeb177767b59dda6`.
Two actual commands, `session list --all --json` and `usage --json`, began
together on a pristine disposable Home outside the repository, with inherited
execution authority removed. Both exited 0 and returned `[]`. Usage warned:
`ledger unavailable — runs are not being recorded`, followed by
`incompatible Loopflow database; delete loopflow.db and rerun the command`.
No database was deleted. The probe stopped after this first counterexample.

Read-only SQL found two succeeded Exec rows and zero agent Run rows, but only
three command journal events: Session-list started/completed, usage completed.
Usage has no started event; its Exec `started_at` equals completion time. Two
rows therefore do not prove preserved command admission. Receipt and copied
binary are under
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo298-first-home-execs-bf8h6n0w/`.
Free disk passed the 64 GiB check. This is an actual isolated CLI reproduction,
not a configured-Home incident or proof of a regression caused by the memo.

Source supports a race hypothesis: `apply_installed_development_sqlite` reads
ledger presence and validates the applied prefix/schema outside the subsequent
exclusive draft transaction. Another initializer may advance between those
reads. `journal::ledger_insert` warns and proceeds, so a later terminal event
can create an Exec whose start was never recorded. Distinguish the migration
snapshot race from the best-effort admission policy during repair. Preserve
real corruption/checksum rejection; neither missing-table-as-empty, suppressing
the warning, nor a terminal-row-count assertion establishes the contract.
The managed worker owns integration and the regression, including no fabricated
start time and no read-only-command Started side effect.

## Session pagination review — 2026-09-28

The in-progress `store/sqlite/sessions.rs::sessions` applies a global
`ORDER BY s.title, s.id LIMIT/OFFSET`; `lf/commands/session.rs::list` applies
`scope_to_repo` only after building those Session surfaces. Consequently a
page filled with another repository's titles can return an empty local list
despite matching local Sessions beyond that global page. This is a concrete
source counterexample, not an executed CLI result. The accepted query contract
requires relevant filters before pagination and before native/payload reads.
Prove with two repositories and a limit smaller than the foreign prefix;
preserve main-checkout/worktree equivalence and unavailable historical ancestry.
Also inspect Desktop's inventory caller: introducing a default limit of 100
must not silently remove the 101st open Session from a refresh or its panes.
Main owns the implementation and coordinated consumer proof.

The subsequent Swift paging loop must specify the page size it tests, rather
than depend on a second implicit copy of CLI's default 100. Its current
`ORDER BY title,id`/offset sequence also spans separate CLI calls: a rename or
insertion before the next offset can repeat an ID, while a completion can skip
one. The 101-static-row fixture alone does not cover that boundary. Retain
inventory identity and panes under such refreshes without adding another
durable snapshot owner. This is a source-level review case; no rendered
duplicate or pane loss was observed. Dense query-plan and latency proof still
must exercise the actual optional-filter/substring-search SQL, not infer indexed
selection merely because filtering moved into SQL.

## Public headless admission proof inspected — 2026-09-28

The supervisor inspected `public-headless-admission/results.json` and the
assertions in `tests/e2e/codex_connect.py::_launch_contract`. Copied CLI SHA-256
`4d72eed3084c5b3bff8f77bfb919697d49d9e2a36ed9012a0735a74f82765eb3`
returned launch exit 0, default empty inventory, one explicitly headless
conversation, and successful rename under the same Session ID. This uses actual
Codex and a synthetic localhost Responses server in a private Home. The receipt
lives at `.lf/tmp/execution-model/public-headless-admission/results.json`.
It predates repository-filter edits, and proves neither continuation/connect,
Flow settlement, final integrated bytes nor configured-model acceptance.

> Execution context: LOO-332. Primary design: `scratch/task-automation.md`.
> Source: `/Users/jack/src/loopflow.discord/scratch/task-automation.md` at `613a66ca8fd99b81b92a902c986dfc1540d6ca10`.
> Stacked on LOO-298 at `d07e569330c8dedceb5dd238ce9b22a7b6137006`. This destination owns ongoing edits.
> Inherited LOO-298 scratch is dependency evidence, not this Task’s assignment.

# Task automation through verified landing

2026-09-28 · Jack Heart · Product · Launch-plan core

## What to build

Keep selected Tasks progressing through their intended Flows to verified landing
and closure, using desktop-managed finite scheduled operations instead of a
resident or a PR watcher. Jack's contract: "make sure things get to landed
successfully (but dont skip intended steps etc)".

Builds on [Data model: one table per user object · LOO-298](https://linear.app/loopflow/issue/LOO-298/data-model-one-table-per-user-object-session-as-a-child-of-run).
Use its current AgentSession/FlowSession/Exec direction and shared claims. Its
implementation is still active; settle integration against the inherited source,
not the older installed runtime. Do not change LOO-298's checkout or live Home.

This is the ambitious single-threaded core. Internal slices ship as one coherent
PR. Discord delivery and VSM policy are separate outcomes; preserve their
requirements in `discord.md` without building them in this PR.

## Demo

Enable Task automation for a repository in Desktop, close the app, and observe
one eligible Task start its intended Flow. A second Task waiting for human review
stays parked. The first reaches `pr land -c`, which returns without a watcher.
Later OS ticks repair failed CI or a required rebase, observe actual merge, and
close the Task in Loopflow and Linear. Reopen Desktop to see the last check,
current work and any blocker. Live deployment remains an explicit demo action;
local builds use private fixtures and never migrate the installed Home.

## Current owners and deletion

- `ops/cron.rs` and `lf/commands/ops/mod.rs`: Wave-specific daily launchd jobs
  and receipts; generalize to the finite repository operation.
- `controller/task/mod.rs`, `ops/task.rs`, shared Flow driver: preserve execution
  order, claims, recovery and review boundaries.
- `ops/pr_landing.rs`: remove the persistent supervisor/wait loop; retain useful
  observations, incident identity and saved merge disposition.
- `ops/task::settle_task_landing`: reuse Task closure and PR-chain settlement.
- `ci-fix` and `pr-land` builtin skills: replace watcher assumptions with the
  finite handoff. `ci-fix` already starts with rebase and returns after arm.
- Desktop invokes shared Rust/CLI operations; no Swift-owned scheduling policy.

## Domain values and operations

Use existing Task, AgentSession, FlowSession, PR, landing intent and CI incident
records. Extend their actual owners only where needed for elapsed CI wait,
repair admission and pending settlement. No second Task cursor or public runtime
object. A schedule records repository, cadence and placed Home; its latest
receipt distinguishes a successful idle check from failed observation.

Proposed internal API shape (adapt names to LOO-298's final types):

```rust
async fn reconcile_repository(store: &Store, repo: &Path) -> Result<TickReport>;
async fn reconcile_task(store: &Store, task: &Task) -> Result<TaskAction>;
fn sync_schedule(spec: &ScheduleSpec) -> Result<InstalledSchedule>;
```

`TaskAction` distinguishes wait, launch/resume, repair, settlement and unresolved
evidence. These are results of existing domain operations, not persistent plans.
Final exported DTOs are explicit required/optional fields with shared fixtures.

## Admission and execution

Use one repository OS job, initially every minute, with per-Task claims. This
cadence and job arrangement are launch-plan defaults, not additional decisions
attributed to Jack. Make the schedule adjustable using the existing scheduling
surface. A tick ends after inspecting and admitting useful work; no waiting loop.

Eligible work is a Task explicitly started/selected for automation or carrying
pending authorized delivery. Unstarted backlog, explicit holds, closed Tasks
and pending human Sessions stay untouched. A Task-bound active Session, Flow
driver or repair prevents a competing launch. Unknown liveness stays unresolved.
Saved but interrupted Flow state is recoverable, not proof of active execution.
Respect Home placement; another Home's work must not be started locally.

When idle, task-operate selects the appropriate Flow only if none is selected,
otherwise resumes/reconciles existing work. It hands execution to the shared
driver and exits. Explicit Flow selections, captured order and review gates
survive every tick. Admission and handoff share authoritative claims: a
preflight read alone cannot prevent two simultaneous launches. Include a test
where a review opens during admission. Admission code must not block its own
already-authorized operator from handing execution to the Flow.

## Landing, repair and closure

`pr land -c` prepares/publishes, requests authorized auto-merge, records
completion-on-merge, and returns. Bare land keeps the Task open; `--next`
preserves the PR-chain disposition. Success means handoff, never false merge.
Later Flow steps requiring an actual merge must remain parked until that fact
exists. Pending delivery remains discoverable after every initiating process
exits. Avoid duplicating `land` and `arm` lifecycle implementations.

The finite check acts on the current PR/head:

| Evidence | Action |
| --- | --- |
| Required checks failed | Admit `ci-fix` when Task admission permits |
| Rebase required, even if green | Admit `ci-fix` immediately; it rebases and resolves conflicts |
| CI pending/queued/absent beyond 30 minutes | Admit diagnosis through `ci-fix` |
| CI pending below deadline | Wait without an agent |
| Green; review or merge pending | Wait or apply only the already-authorized merge action |
| Provider observation unavailable | Keep the exact gap visible; do not infer failure/merge |
| Merged | Apply saved settlement intent without an agent |
| Closed without merge | Keep unresolved; never complete as successful |

CI wait timing survives ticks/restarts and includes expected checks that never
start. New head or explicit CI rerun starts a new attempt without deleting
history. An unchanged incident cannot launch duplicate repairs. A failed repair
with an unchanged external blocker stays visibly blocked until new evidence or
explicit retry. Bound consecutive timeout-only reruns; begin with one automatic
rerun per incident before surfacing the blocker. Do not invent commits to reset
the timer. Green-but-needs-rebase remains actionable independent of this clock.

`ci-fix` rebases, repairs, verifies, publishes and restores the saved disposition,
then returns. It does not restart the feature Flow or skip its earlier reviews.
Task closure retries partial local/Linear settlement after authoritative merge.
Merged-but-closure-pending remains distinct from complete. Repeated settlement
converges without restarting implementation or duplicating completed effects.

Existing taskless landing requests also need a finite observation path: inspect
recorded repository landing intents with their existing PR claims, but perform
no Task closure. Do not retain a watcher solely for taskless callers.

## Internal slices

1. **This slice: finite reconcile and landing handoff.** Reuse observations,
   claims and settlement; prove `land -c` returns and a later invocation repairs
   or closes correctly. Preserve generic Flow merge-dependent boundaries.
2. Add Task admission and task-operate handoff; prove active/review exclusions,
   recovery and overlapping ticks against the real shared driver.
3. Install/manage the repository job through Desktop and shared `lf` operations;
   expose enabled state, cadence and latest outcome/error. Reopening the app or
   resyncing must not duplicate jobs; disabling stops new admissions.
4. Update user docs and builtin delivery skills, run affected checks once, and
   demonstrate the full installed path through the Flow's human demo gate.

## Done when / forbidden near-misses

Prove busy Tasks, open human Sessions and terminal Tasks never get a competing
driver; two ticks admit at most one operator/repair. Prove deadline persistence,
rebase despite green CI, stale-head rejection, no repeated unchanged repair,
and interrupted settlement recovery. Then prove one installed job makes progress
with Desktop closed, including merge followed by Task closure after `land`
has exited. Mocks are useful focused evidence, not that installed demonstration.

No lfd, Wave resident, replacement keeper, watcher, parallel Flow cursor,
automatic review approval, Swift lifecycle implementation, or accidental
automation of unselected backlog. A published PR, green CI, an exited Flow or
a launched cron alone does not satisfy this core.

## Evidence ledger and handoff

Source design was checkpointed at `ce0263277` in `loopflow.discord`.
The attempted direct rebase encountered unrelated main commits #1307/#1308;
it was aborted intact. Prepare this core directly stacked on LOO-298 through
`lf task prepare --stack-on`; record the actual inherited commit before launch.
No implementation or live schedule activation was performed by launch-plan.
Implementation should append dated proof here while preserving the full scope.

### 2026-09-28 — finite landing handoff, first implementation pass

Compared with `0f5ff18c8e1095c33f9e376d43de5cbd78441661` in this checkout.
Jack's complete scope above remains in force. This pass is retained working
code, not completion of LOO-332 or an installed demonstration. No commit,
publication, live job, real provider request, or installed-Home migration was made.

**Switched consumers.** Both CLI `pr land` and `pr arm` now prepare through the
same existing arm operation, retain the existing landing intent, and return.
`pr reconcile` reads the repository's pending landing rows and performs one
observation, at most one confirmation, and at most one CI repair. Taskless
requests use this same path. The former `watch_armed_pr`, `watch_armed`,
`wait_for_landing`, `supervise_pr_landing`, sleep-between-observations, and
watcher-only repair-output scan are deleted. A repair still runs in the calling
process until that finite provider invocation finishes; detached admission is
not yet implemented.

The existing landing generation and worktree lock now cover one check. A
contending check returns immediately, and a canceled asynchronous repair retains
the lock until its blocking worker exits. Release fences the generation. CI
response admission updates only an incident with no prior response; repeated
unchanged failures stay blocked instead of launching the previous unbounded
repair loop. Blocked deliveries remain observable, including a later merge.

Mechanical Flow `pr land` stores its exact landing ID on its operation Run.
The common executor can return Waiting for an operation, preserving the same
captured position and handoff receipt across invocations. Only that landing's
settlement releases the next step. This uses one new nullable FK on `runs`,
in the `link_landing_operation` draft; it creates no second cursor or delivery
owner. The returned process status means handoff, not merge.

**Review corrections.** Landing persists verified merge evidence before Task
settlement. Settlement reuses the exact already-merged Task PR if an earlier
check rotated the active pointer. A still-unsatisfied completion gate or pending
Linear writeback is reported as closure pending, retaining the landing for retry.
The earlier helper could return success after local completion with pending
writeback. These source corrections still need a dedicated interrupted
local/Linear settlement test; the retained Task test proves only the merged
evidence prerequisite and local completion.

**Observed proof.** All Rust commands used a disposable default Home/database,
cleared inherited `LF_*`/`LOOPFLOW_*`, four workers, nice +10, a 900-second bound,
and no incremental build (`CARGO_PROFILE_DEV_DEBUG=0`). Providers/GitHub were
stand-ins. Resource preflight first failed at 63.8 GiB; supported recovery
restored the floor by removing an inactive checkout's allowlisted build output.
It retained active builds. The uv-cache prune timed out on its lock and removed
nothing; later preflights passed.

- `cargo test -p loopflow --lib -- ops::pr_landing::tests handed_off_landing
  store::sqlite::pr_landings::tests store::sqlite::ci_incidents::tests
  watched_landing_completes --test-threads=4`: 11 passed, one new fixture failed
  because its human review lacked a stable ID. The corrected fixture next failed
  because it attempted an unclaimed landing update. It now uses the real claim.
  Final focused repeat of landing and Flow behavior: **8 passed**. The three
  existing landing-store tests and local Task-completion test passed earlier;
  the CI-incidents filter selected no tests.
- `cargo test -p loopflow --test land_tests lf_pr_land_returns --
  --test-threads=1`: **1 passed**, covering three actual-CLI scenarios:
  immediate merge, repair then later merge, and blocked repair. Each initiating
  `land` process had exited before a separate `reconcile` process ran. This
  test is taskless; it does not prove `-c` with a live Linear provider.
- The saved-Flow proof resumes the same operation twice while pending, verifies
  its Run and cursor remain unchanged, then records a claimed merge and reaches
  the following human review without launching an autonomous provider.
- `cargo clippy --all-targets -- -D warnings`: **passed** after fixing an
  inherited unused Session import and five needless Wave-command returns.
  The first test compile also required deleting an inherited removed `Cli.repo`
  test initializer. These are local integration fixes, not parent acceptance.
- The retained disposition and revoked-auto-merge checks were also run. The
  disposition assertion first omitted the existing shell quoting around `--next`;
  that fixture expectation was corrected without changing production quoting.
  Final results: **both passed**, with only the failing disposition test repeated.
- `uv run --project website --extra test pytest
  website/tests/test_readme_index_sync.py -q`: **1 passed**.
- `git diff --check`: passed. Changed Rust files were formatted. Global
  `cargo fmt --all -- --check` still reports inherited formatting in
  `ops/flow_session.rs`, `store/sqlite/durable.rs`, and four integration tests;
  formatting-only edits in unrelated files were restored.
- `uv run python scripts/check_migrations.py`: **blocked** because
  `0.12.24.001_release.sql` exists on `origin/main` but is absent from this
  LOO-298-stacked branch. No released migration was copied, rewritten, or applied
  to the installed Home. Integration against the parent's final schema remains
  required before delivery.

**Comparable size.** Rust production changes: **+431 / −451, net −20** lines
against the initial HEAD. Counted changed `src/**/*.rs` before their test module;
`lf/commands/ops/mod.rs` was counted whole because its unchanged test modules are
interspersed with production. Excluded integration tests, test-only changes,
Markdown, generated files, and SQL. The new draft adds **6 SQL lines** separately.
Most deletion is the persistent watcher; the finite reconciler and Flow binding
take its place. The large overall test deletion replaces watcher-loop tests with
separate-invocation behavior; disposition and revoked-request proofs remain.

**Still required in this slice and the complete design.** Durable CI deadlines,
explicit rerun identity and retry policy, green-but-needs-rebase observation,
and interrupted local/Linear closure proof remain. Task-wide admission is not yet
connected: the current PR claim serializes competing landing checks, not Task
Flows or independent Sessions. Do not activate scheduling or treat this branch
as ready to ship before that shared admission boundary, detached repair launch,
Desktop management, and the human installed-path demo are implemented and proven.

Compression (2026-09-28): removed unused repair-continuation/result plumbing,
CI repaired-head writers, terminal predicate, and duplicate CLI land/arm dispatch;
confirmation now precedes one action dispatch, and the Flow outcome is infallible.
Focused landing/Flow/store proof: 14 passed; CLI land/arm proof: 2 passed (private
Homes and provider stand-ins). Clippy and global format check passed; formatting
also normalized six inherited files. Full Task and installed-demo gaps above remain.

### Review: slice 1, finite reconcile and landing handoff (2026-09-28)

Scope: `d07e56933..ff380db2d` plus the review repairs below. Providers and
GitHub are stand-ins in private Homes; nothing here is installed acceptance.

| Claim | Result | Executed evidence |
| --- | --- | --- |
| `pr land`/`pr arm` hand off and return; no watcher remains | pass | `land_tests`: 20 passed. `watch_armed_pr`, `wait_for_landing`, `supervise_pr_landing`, poll intervals: zero references in `rust/` |
| Later check repairs once, settles merge, never completes closed-unmerged | pass | 12 reconciler tests, 3 landing-store tests |
| Flow stays at `pr land` until that exact landing merges | pass | `handed_off_landing_keeps_the_saved_flow_before_its_next_review` |
| Task completes only from merged evidence | pass, local only | `watched_landing_completes_task_only_from_merged_pr_evidence` |
| Interrupted local/Linear settlement retries | **gap** | Source path exists; no test interrupts it |
| Rebase required while green admits `ci-fix` | **gap** | `LandingObservation` has no such evidence; not implemented |
| 30-minute CI deadline, rerun identity, retry bound | **gap** | Not implemented; pending waits forever |
| Migration history | **blocked** | `check_migrations.py`: `0.12.24.001_release.sql` is on `origin/main`, absent here. LOO-298 is not on main (`c512813b5`) |

Library run: 16 passed (reconciler, store, Flow, Task). Clippy all-targets,
`cargo fmt --check`, `check_architecture.py` and `git diff --check` pass.

Repairs made in review:

- A blocked landing kept its old reason after checks recovered. Pending or
  passing evidence on an armed head now returns it to watching.
  `recovered_checks_clear_a_block_without_another_repair` failed first
  (`Blocked != Watching`), then passed.
- Four Task-bound `land_tests` failed at the base too: the shared fixture's
  PM snapshot lacked LOO-298's required `flow`/`status`. Fixture repaired in
  `tests/support/mod.rs`. The earlier ledger's CLI proof ran one filtered
  test and did not surface this.
- `docs/lf.md`, `docs/architecture/delivery.md`, `architecture-reference.md`,
  `getting-started.md` and `waves.md` still described the watcher. Rewritten
  for handoff and `lf pr reconcile`.

Measured against `d07e56933`, Rust production lines before each file's first
test module (`lf/commands/ops/mod.rs` whole): **+504 / −612, net −108**.
Excludes tests, docs, generated files and the 6-line SQL draft.

Findings carried forward:

- Nothing wakes a Flow parked at a handed-off landing. `lf pr reconcile`
  settles the landing; the Flow advances only when something drives it again.
  Slice 2 admission owns this.
- A repair runs inside the reconciling process. A scheduled tick therefore
  lasts as long as `ci-fix`. Detached launch is slice 2.
- A blocked landing makes `lf pr reconcile` exit nonzero on every tick until
  evidence changes. Visible, and noisy for a one-minute schedule; the schedule
  receipt in slice 3 must distinguish this from a failed observation.
- Flow `pr land` records the landing twice (`arm`, then `record_armed_pr`).
  Idempotent; remove when `arm` returns the landing.
- `docs/architecture.html` is stale against LOO-298's `architecture.md`, and
  `test_portable_architecture` asserts a phrase that rewrite removed. Left for
  LOO-298; regenerating here would carry its content into this PR.

Not published: three slice claims are gaps and the migration check cannot pass
until LOO-298 integrates main. Next cut: needs-rebase evidence and the durable
CI deadline in the reconciler, with an interrupted-settlement test, then Task
admission. Proof: reconciler tests for each row of the evidence table, and one
CLI test where `land -c` on a Task exits and a separate `reconcile` closes it.

### 2026-09-30 — integrate main through the owned sync

Merged pinned main `6c7335607`, resolving against Loopflow's stack comparison
tree. Finite landing now binds the exact mechanical `flow_events` start instead
of the removed Run table. Main's Session/Exec attribution, `cmd`/`sync` syntax,
merge-queue evidence, published-head reuse, and standalone cleanup are retained.
A pending taskless Flow keeps its checkout and resumes after reconciliation;
its successful operation is not replayed. Release calls the finite reconciler
and retains its preparation checkout until merge settlement, while its existing
release lifecycle owns waiting. Removed two watcher-only async history readers.

Review found and fixed the release caller of the deleted watcher, the telemetry
caller's changed return type, and the Flow wait message that assumed all waiting
was human review. The first continuation committed conflict paths but reported
unstaged integration edits; those require a follow-up checkpoint before owned
continuation completes. No push, installation, live provider or schedule action.
The remaining automation slices and installed demonstration above remain open.

Checks: `cargo test -p loopflow --lib -- ops::pr_landing::tests handed_off_landing --test-threads=4` (15 passed); `cargo test -p loopflow --test land_tests lf_pr_land_returns_before_later_checks_repair_and_observe_merge -- --test-threads=1` (1 passed, six scenarios); `cargo test -p loopflow --test release_tests release_run_repairs_failed_checks_before_tagging -- --test-threads=1` (1 passed, two queue states); private Homes/provider fixtures, format and diff checks passed; broader verification remains Gate/CI-owned.

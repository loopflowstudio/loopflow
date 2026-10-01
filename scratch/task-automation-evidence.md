# Task automation — historical implementation evidence

Retained unchanged from the pre-restart plan on 2026-09-30. These dated
receipts establish only their stated scope; the current recommendation and
remaining work live in [task-automation.md](task-automation.md). Historical
execution instructions below are evidence, not the current plan.

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
unstaged integration edits; those were checkpointed as `136e1de63`. A final
`lf sync --continue` reports no sync in progress. The checkout is clean, has no
unmerged paths or MERGE_HEAD, and contains the pinned target as an ancestor.
No push, installation, live provider or schedule action.
The remaining automation slices and installed demonstration above remain open.

Checks: `cargo test -p loopflow --lib -- ops::pr_landing::tests handed_off_landing --test-threads=4` (15 passed); `cargo test -p loopflow --test land_tests lf_pr_land_returns_before_later_checks_repair_and_observe_merge -- --test-threads=1` (1 passed, six scenarios); `cargo test -p loopflow --test release_tests release_run_repairs_failed_checks_before_tagging -- --test-threads=1` (1 passed, two queue states); private Homes/provider fixtures, format and diff checks passed; broader verification remains Gate/CI-owned.

# LOO-355: composable Task lifecycle

Jack Heart requested lifecycle composition, issue-addressed Task actions, and
cancellation of open issues left outside current chapters (2026-09-30).

## Contract

Task owns the Linear outcome, local disposition and execution; PR owns GitHub
and its retained Task PR record; worktree owns local checkout and branch plus
remote branch. Abandonment composes downward. Completion composes landing with
the existing authoritative-merge completion gate. Lower operations explicitly
report retained higher-level state. Errors remain errors; retry observes already
completed effects. Missing checkout/remote branch is a supported recovery case.

Never infer cancellation from missing membership. Protect completed issues,
current-chapter sweep members, dirty checkouts without explicit force, and live
or unknown execution. Preserve Task/PR/Run history. Sweep is dry-run by default;
report open PRs and execution as exclusions before applying safe candidates.

## Delete — do not maintain

- Removed Task-only `rebase` dispatch; `task sync ISSUE` delegates to the sole
  integration command after the 2026-09-30 merge from main.
- Removed the terminal-worker early return that strands its claim. Preserve the
  bound worker fence, captured Flow until settlement, and authoritative merge gate.
- Removed issue deletion's standalone placed-Task path: cancellation or completion
  cleanup now precedes provider trash. Preserve history and confirmed-deletion
  retry evidence. No terminal-outcome reopening follows from execution recovery.

- Removed the separate Done-worker finish path and the single-caller `settle_pr`
  wrapper. One worker settlement path now retires both completed Tasks and final
  Flow steps; checkout cleanup still follows provider stop and exact-claim settlement.

## Current implementation

The action/ownership inventory is in `docs/lf-reference.md` under Lifecycle action
inventory and copied into the prepared PR body. Task cancellation now uses the
same PR abandonment operation as `pr abandon`; that operation uses the same
checkout/branch deletion as `wt delete`. Missing refs and checkouts support retry.
Task repository selection follows retained Wave ownership before requiring the
caller to be in Git. PR and sync operations are reachable by issue ID.

Sweep includes archived Projects and issues, defaults to preview, rechecks chapter
membership on apply, and reports exclusions and incomplete operations. It reads
issue attachments too: an open PR without a retained Task PR record is excluded,
not inferred absent. GitHub branch matches include the source repository, so
same-named branches in other forks are not closed. Attachment URLs cannot choose
a host for GitHub credential-bearing reads.

Cancellation and sweep share preparation and application. Preparation reads the
Task PR records once and checks all unmerged records, including abandoned records
whose GitHub PR reopened. Sweep preparation never releases worker claims. Apply
uses the final fresh chapter-membership result directly before effects, instead
of repeating the entire preparation and membership lookup. This removes the
preview/apply discrepancy found during compression without changing the existing
transactional claim fence, remote deletion lease, or partial-failure recovery.

Completion cleanup now runs at worker settlement, after provider shutdown and
exact-claim retirement. Explicit completion retries cleanup without recreating a
checkout or rerecording success. PM writeback uses the durable Wave repository,
so a removed Task checkout does not prevent retry. Merged branch tips must still
match the recorded head; dirty work and advanced local/remote branches survive.
Unclaimed execution retires with completion; claimed execution retains its saved
boundary until settlement. Empty successors retain abandoned branch history for
cleanup retries. Standalone landing shares the same checkout deletion. Bare Task
landing retains and reports the open Task's checkout for continued delivery.

Deletion now cancels unfinished placed work or cleans completed work before
provider trash. Recovery uses the existing checkout/run commands and idempotent
lifecycle retries. It does not reopen terminal outcomes. The final action table
is maintained in the reference guide and prepared PR body.

## Remaining work

- After landing and installation, demonstrate abandonment on a disposable
  configured Task across Linear, the owning store, GitHub and Git.
- Then run the configured Wave sweep preview and apply eligible cancellations,
  including LOO-309 and LOO-329. The latest supervising steer explicitly places
  these proofs after landing through installed `lf`; v0.12.27 supersedes the
  earlier parser-version observation below.
- Caller owns publication, installation and Flow navigation. LOO-355 remains
  unfinished until the configured acceptance above succeeds.

## Proof and review evidence (2026-09-30)

All Rust tests ran with inherited `LF_*` authority removed and `LF_BIN` pinned to
the source CLI path, against disposable stores and Git remotes. Linear and GitHub
responses were simulated; no provider credentials or production issues were used.

- `cargo test -p loopflow --lib ops::wt::tests -- --test-threads=1`: 2 passed.
  Proves both branches and checkout disappear, retry without refs/checkout,
  dirty/default-checkout preservation and remote advancement rejecting deletion.
- `cargo test -p loopflow --lib task_abandon -- --test-threads=1`: the composed
  cancellation and store-claim tests cover issue, branch and checkout selectors,
  an issue-addressed caller outside Git, provider refusal, partial GitHub failure,
  explicit lower-layer notice, retained history and cleanup retry. Final fence
  verification passed both tests, including claim rejection while cancellation
  intent is pending.
- `cargo test -p loopflow --lib task_sweep_ -- --test-threads=1`: 1 passed.
  Preview leaves issues unchanged, current/terminal issues survive, an untracked
  open PR is excluded, eligible planning work is canceled and apply retries.
- `cargo test -p loopflow --lib task_lifecycle_commands_ -- --test-threads=1`:
  1 passed. Lower PR flags retain their values and wt remove/rm are absent.
- `cargo test -p loopflow --lib task_completion_ -- --test-threads=1`: 10 passed
  before the final cancellation-only changes. These preserve existing completion
  behavior; they do not prove the unimplemented landing-cleanup symmetry.
- `cargo clippy --all-targets -- -D warnings` passed after the final worker
  intent fence. Formatting and diff checks also passed.

Earlier failed attempts exposed an invalid review fixture without a node ID,
missing current-Project fixture responses, duplicate fixture Project names and
macOS /var versus /private/var path identity. Fixtures were corrected. A final
compile used an unavailable direct url dependency; it now uses the existing
reqwest URL re-export. Those failures are not passing proof.

The simulated review changed the implementation: propagate provider/Git failures;
read back cancellation; preserve primary/dirty/advanced remote work; resolve
historical branches after cleanup; refuse claimed workers transactionally; use
existing intent to prevent a claim racing the provider request; inspect untracked
PR links; and match source repository identity. No full gate or CI ran.

Read-only configured GitHub observations: PR #1299 and PR #1318 both returned
CLOSED, with headRepository loopflowstudio/loopflow. This confirms the PR facts
only. The earlier installed CLI observation was 0.12.26 and rejects task abandon and task sweep;
it provides neither a configured sweep result nor Task cancellation evidence.

Compression verification (2026-09-30): the three abandonment/sweep/store tests
passed, including reopened historical PR exclusion in both preview and apply,
and membership changing into the current chapter during preparation. The other
shared planning-fixture consumers and lifecycle parser passed 13 tests; one
subprocess-only fixture entry was intentionally ignored. Commands used the same
isolated environment described above:

```text
cargo test -p loopflow --lib -- task_abandon task_sweep_ --test-threads=1
cargo test -p loopflow --lib -- task_planning_tests task_lifecycle_commands_ --skip task_abandon --skip task_sweep_ --test-threads=1
```

The review also removed temporary argument copies from lower-command validation
and corrected the documentation's implied issue-ID inference for landing.
All-target Clippy, formatting and diff checks passed after compression.
At that compression checkpoint, completion/deletion composition and configured
live acceptance remained unfinished. The subsequent pass above supersedes the
implementation gap; configured acceptance remains after landing.

## Completion/deletion review (2026-09-30)

`lf sync --plan` selected merge_target; `lf sync` merged v0.12.27. The owned
resolver reconciled `docs/lf.md` and reported the Task-only `rebase` dispatch
left behind by main's rename. This pass replaced it with `task sync` and removed
the old spelling rather than adding an alias.

The simulated review changed four boundaries:

- Worker settlement must survive Task success recorded inside its own turn.
  Terminal success no longer strands the exact claim; provider stop failure
  prevents cleanup. New worker claims still require Ready Work.
- Cleanup retries need branch identity after an empty successor is retired.
  The completion transaction now retains that abandoned record instead of
  deleting it. Task success and Flow settlement remain separate events.
- A deleted checkout cannot be the context for PM writeback or completion retry.
  Writeback uses retained Wave ownership; a Done Task's absent checkout is
  expected when rendering its completion gate.
- Provider issue trash cannot strand placed execution and Git state. The public
  deletion operation now composes cancellation/completion cleanup first, then
  invokes the existing confirmed-trash recovery path.

All new behavioral proof remains disposable/synthetic. The new cleanup fixture
initially failed because its PR lacked reviewer copy, then because it attempted
to change the immutable base through the general update writer; it now uses the
existing base-healing and merge-settlement writers. A compile attempt also
passed a timestamp without the required Option. Those failures are retained
observations, not passing proof. No live provider credentials were used.

Final focused evidence for this pass:

- `cargo test -p loopflow --lib -- task_completion_ task_deletion_planning_ --test-threads=1`:
  15 passed, including public completion retry and snapshot after checkout removal,
  worker deferral, dirty/local/remote follow-up preservation, retained history,
  planning deletion confirmation retries and prior completion/writeback behavior.
- `cargo test -p loopflow --lib -- task_completion_cleanup_ task_abandon_and_delete_ task_sweep_ task_lifecycle_commands_ --test-threads=1`:
  cancellation/deletion, sweep and parser passed; cleanup's fixture failure was
  subsequently corrected and passed in the final command above. Deletion was
  exercised from outside Git and retried after issue trash.
- `cargo test -p loopflow --test pr_tests completing_land_discards_an_empty_successor_without_a_controller -- --test-threads=1`:
  1 passed, proving completion retains the retired successor for cleanup recovery
  without creating an empty GitHub PR.
- `cargo test -p loopflow --lib -- task_completion_cleanup_ pr_landing_observes_ci_fix_publication --test-threads=1`:
  the landing supervisor proof passed; the earlier cleanup fixture failed and is
  superseded by the final focused proof above.

Commands removed inherited `LF_*` variables and pinned `LF_BIN` to the source
checkout's compiled CLI. No affected-suite gate, full repository suite or hosted
CI ran. Configured acceptance remains the caller's post-installation obligation.

Final `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
`git diff --check` passed after the public cleanup-retry proof was added.

## Final compression (2026-09-30)

Worker settlement now shares its timestamp, bounded progress summary and exact
claim retirement for final Flow steps and Tasks completed inside a turn. The
existing final-step regression also proves a Done Task with remaining Flow steps
retires once before reporting incomplete cleanup, preserves its checkout and
never reopens success. Lifecycle repository resolution reuses `owning_wave`;
PR settlement no longer passes through a single-caller wrapper. The command
reference removes obsolete inferred-owner examples and uses issue-addressed Task
PR operations or checkout-addressed PR/worktree/commit commands directly.

Focused isolated proof passed 16 tests:

```text
cargo test -p loopflow --lib -- finished_task_or_final_skill_ claimed_autonomous_boundary_settles_once_ task_completion_ task_abandon task_sweep_ --test-threads=1
```

Inherited `LF_*` authority was removed and `LF_BIN` pinned to this checkout's
compiled CLI. Provider responses remain simulated, with disposable stores and
Git repositories/remotes. Configured post-installation acceptance remains
outstanding; this pass performs no publication, installation or live cancellation.

Final `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
`git diff --check` passed. No broader gate or hosted CI ran.

## Gate review (2026-09-30)

Jack Heart's latest supervising steer authorizes shipment and assigns the live
disposable-Task proof and configured sweep to the supervisor after the next patch
release installs the change. This gate does not perform those provider mutations.

Review found and fixed these implementation/test boundaries:

- Checkout deletion now accepts an absent directory whose Git registration still
  exists. It retains the normal deletion lease and branch-head checks, removes
  the registration and both branches, and propagates filesystem inspection errors.
  The regression uses a disposable checkout and bare remote. Selection uses local
  worktree facts instead of refreshing every checkout's PR/network state.
- An old parser test still classified `task abandon` as removed. The Linux CLI
  deletion fixture also expected the previous retained-PR wording and allowed
  unfinished primary-checkout deletion. It now names the actual primary branch,
  checks cancellation refuses that checkout before provider trash, and retains
  the completed-history and planning-only deletion proofs.
- Broader tests still expected abandoned Tasks to keep saved execution and empty
  successors to disappear. They now prove a stale restart cannot restore the Flow
  and the retired successor keeps its cleanup identity without remaining active.
- Watched landing now verifies checkout/local/remote deletion and compares the
  repair head with the durable landing record, rather than running Git in the
  deleted checkout. Release cleanup-lock and argument-order fixtures use `wt delete`.

The initial changed-aware gate (`--base 12013dae4fadc4946b885309ebcc22f1061e6493
--reuse-passing`, no evidence reused) passed architecture, formatting, Clippy and
website checks (78 passed, three skipped). Rust stopped on the stale parser
expectation: 827 passed, one failed, 12 skipped, 1,138 not run. Its receipt remains
failed at `.lf/tmp/gate/run-47778`; later checks do not rewrite that result.

The all-target rerun used `scripts/materialize_rust_tests.py -- cargo nextest run
--all --build-jobs 4 --test-threads 4 --no-fail-fast`: 1,960 passed, seven failed,
12 skipped. Six failures were the stale lifecycle/command expectations corrected
above. The seventh was the unchanged screenshot timeout fixture: its fake-browser
PID file was absent after its 500 ms deadline (`screenshot.rs:465`). This is an
observation of fixture startup timing, not a live browser failure. The failed run
remains `.lf/tmp/gate/final-rust.log`; no all-green full-suite result is claimed.

Rust runs clear inherited `LF_*` variables and pin `LF_BIN` to this checkout's compiled CLI;
materialization uses a disposable source snapshot. The Linux fixture uses an
isolated container, synthetic Linear HTTPS and no host Home or credentials.

Final focused verification ran all seven failed cases plus the parser and shared
checkout deletion tests, in the materialized source snapshot with one test worker.
Ten passed; the screenshot fixture failed again with the same absent PID file.
All lifecycle failures from the broader run are corrected and passing. The
screenshot implementation and test are unchanged by this branch; its cause remains
unresolved outside this gate's lifecycle scope. No retry, timeout extension or
assertion weakening was added. The full gate remains red on that existing test.

```text
uv run python scripts/materialize_rust_tests.py -- cargo nextest run --all --build-jobs 4 --test-threads 1 --no-fail-fast -E 'test(task_worker_stop_observation_) | test(empty_pr_is_retired_) | test(lf_pr_land_waits_) | test(active_candidate_preparation_) | test(active_tagged_publisher_) | test(reorder_args_moves_flags_to_nested_owners) | test(timeout_reaps_) | test(consolidated_commands_) | test(ops::wt::tests)'
```

The Linux-only `cargo test -p loopflow --test task_deletion_tests --jobs 1 --
--nocapture` passed in a disposable `loopflow-task-proof-fixture:local` container:
one test, exercising the compiled CLI through synthetic Linear HTTPS. The container
was removed; host Home, provider credentials and installed state were not mounted.
The fixture tests completed primary-checkout retention, unfinished primary-checkout
refusal, confirmed-trash retries and planning-only deletion. It does not replace
the supervisor's configured abandonment/sweep acceptance.

Logs: `.lf/tmp/gate/final-focused.log` and `.lf/tmp/gate/linux-deletion.log`.
Python Ruff checks and formatting passed. No hosted CI, publication, installation,
Task completion or live provider mutation was performed. Unchanged Swift/UI and
the full hosted matrix remain with CI; the affected website suite already passed.

Final `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`,
and `git diff --check` passed after the test corrections.

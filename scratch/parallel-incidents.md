# Retained incidents: publication and refused-start cleanup

2026-09-28 · LOO-298 · Bounded read-only audit requested by Jack Heart.
Inspected dirty source over `03330279f34a75480465a6c8239935c3ac4638ee`;
concurrent implementation remains owned by the managed worker. Read AGENTS.md,
TESTING.md, the working design, parallel allocation and retained incident notes.
No build, test, branch executable, installed Home, provider request or Git mutation
ran. This note is the only authored file; no patch is supplied.

## PR #1296: association recovered; cause unknown; recording gap remains

**Retained observation.** Comment `df090914-0a74-4507-b808-7348df2fb38b`
reported a created PR without its Task PR association. The
[earlier supervisor inspection](supervisor-incident-disposition.md) subsequently
recorded Task PR `pr_64160469a52645eaafa913a5e024d2db` holding GitHub **1296**,
its URL and head `8ccb0bde9ff84d843d4244add77986c62a59e89c`. This audit did
not repeat that read. Local notes do not establish the original command,
failure output, write ordering or repair mechanism. No historical reproduction
or attribution of its cause is justified.

**Current source.** Paths below are under `rust/loopflow/`:

- `src/ops/pr.rs::create_or_update_pr` records the publication request before
  GitHub create/edit, then attaches the returned identity. However, successful
  `create_pr` is followed by fallible `find_open_pr` and possibly
  `mark_pr_ready`, before `attach_task_github_pr`. A failed follow-up read exits
  despite the known created URL. URL-number fallback covers `Ok(None)`, not
  `Err`. Existing-PR readiness failure also precedes attachment.
- `src/ops/task.rs::attach_task_github_pr` resolves the Task again, sets the
  GitHub identity in memory, performs `link_pr_to_linear`, then persists it with
  `update_task_pr`. A returned Linear error is retained rather than propagated,
  but interruption while linking can still precede durable GitHub attachment.
  `PrOpened` is appended afterward; failure there means the row can already
  contain the association despite command failure.
- `reconcile_task_pr_observation` deliberately skips remote inspection without
  a persisted GitHub number. Status alone therefore does not repair this gap.
  Publication retry can discover and attach the existing open PR.
- `src/store/sqlite/children.rs::update_task_pr` now preserves `parent_pr_id`
  during unrelated updates. Keep this integrated stacking correction and the
  locked base reread; no additional parent-equality condition is proposed.

These are source-reachable failure windows, **not an explanation of PR #1296**.
Exec/AgentSession history may expose the failing command; it does not commit
the Task PR observation or remove the provider/local transaction boundary.

**Smallest proof and repair direction.** Extend
`tests/task_pr_authority_tests.rs::valid_authority_publishes_and_records_the_pr`.
Its local bare remote/private Home and simulated `gh` already prove creation
with an empty subsequent list, including stored GitHub number. Add a stateful
fixture: first list empty, create returns a valid URL, next list fails. Assert
that the acknowledged identity survives locally; retain the read error as
degraded evidence. Retry with that same PR visible and prove one creation and
the same Task PR. The present ordering predicts the first assertion will fail;
it was not executed here. Preserve the current parent and publication request.

The narrow correction belongs in existing publication/attachment writers:
persist acknowledged identity before optional post-create enrichment/Linear
linking, while preserving merge-request coordination and reporting failures.
Do not manufacture a head SHA or turn a failed read into an empty result. A
crash before the creation response remains uncertain and requires retry
reconciliation. No new publication ledger or history-based reconstruction.
Main owns this shared code and the regression; no untested patch is added.

## Cancellation/refused starts: retained deletion proof; settlement deferred

**Reported trigger.** Comment `bb5410ba-cc7a-4247-824e-4ec9c4f3fda6` names
duplicate issues LOO-299–302, including issues left by refused stacked starts.
The retained summary does not supply each failing invocation. LOO-305's later
accepted scope replaced cancel/abandon with `lf task delete` and explicitly
deferred process settlement. Do not restore the earlier broader cancellation
proposal or duplicate the ongoing stacking repair.

**Retained outcome.** Local commit
`4a14c0a47dc6e04be9668fb72b737828565a931d:scratch/task-deletion-installed-demo.md`
records deletion of all four issues, successful retries and refreshed
Wave/roadmap absence. LOO-299/300 were local abandoned Tasks; LOO-301/302 had no
local Task. It did not prove process termination or preservation of live
checkout files. That demo advanced installed drafts and broke the older CLI;
later restrictions forbid repeating it. These are historical results, not
current configured acceptance.

**Current source dispositions.**

- `src/ops/task.rs::task_create` supplies `prepare_task_creation` through
  `src/ops/task_pm.rs::create_and_load_task` to
  `src/ops/pm.rs::pm_create_task_idempotent`. Preparation precedes `create_item`;
  marker recovery handles uncertain create responses. Post-create allocation
  failure retains the issue and reports its retry. Preserve this behavior;
  neither Exec logging nor retrospective deletion replaces preflight.
- `task_delete` delegates to `src/ops/pm.rs::delete_task`: fresh provider
  ownership, acknowledged/trash-confirmed deletion, local confirmation and
  planning refresh. `src/store/sqlite.rs::confirm_task_deletion` atomically
  abandons Ready Tasks and records deletion, retaining terminal outcomes.
  It does not stop a worker or provider, or release a Flow claim.
- Existing focused proofs are
  `task_creation_refusal_preserves_inventory_and_marker_retry_reuses_provider_title`,
  `task_creation_snapshot_failure_retries_without_starting_backlog`,
  `task_preparation_rejects_unpublished_parent_before_allocating_child`, and
  the four `task_deletion_planning_*` tests. The real-binary synthetic Linear
  proof is `tests/task_deletion_tests.rs::task_delete_binary_reconciles_provider_and_local_history`
  (Linux only; private Home and fixture CA). None is newly executed here.

**Separate unresolved process counterexample.** The same historical commit's
`scratch/registered-execution-evidence.md` and
`scratch/released-exec-reproduction.patch` retain a real `sleep` child with an
exact PID/start receipt. The fixture finished its Run and released its claim
*before* calling `stop_task_worker`; discovery was empty and stop succeeded,
but the child remained Live. The recorded test failed once in 6.20 seconds
after cleaning its owned child. It used synthetic attribution/receipts and
did not delete a provider issue or run an actual agent.

Current `src/ops/task.rs::stop_task_worker` still returns immediately when no
claim exists. Existing stop tests cover a claim released **after** stop has
captured its owner; that does not cover the retained earlier-release case.
The new `execs` causal tree and Session driver/provider generations do not alone
establish Task-wide process ownership or death. Causal ancestry grants no signal
authority; an lf process ending does not prove its native provider ended.

The smallest local settlement investigation is to adapt that retained fixture
to current Session/Exec constructors: retain exact native PID/start identity
and provider generation, release the Flow claim before stop, observe whether
the identified process survives, and always clean up only that fixture child.
Also retain unknown/reused-PID and replacement-client refusal. Existing
`src/lf/commands/util.rs` tests `provider_client_stop_preserves_unknown_process_evidence`
and `provider_client_stop_rechecks_identity_before_signaling` supply those
native ownership boundaries. Do not apply the historical patch verbatim: its
Run/claim APIs predate this conversion. It is a counterexample to settlement,
not a mandate to make `task delete` signal processes outside its accepted scope.

## Integration checks, not results

Main can run these existing focused proofs through its serialized private-Home
runner after resource preflight (scrubbed LF/LOOPFLOW authority, nice +10,
four Cargo workers per TESTING.md). No configured services are needed:

```sh
uv run python .lf/tmp/cut-i/run.py incident-publication cargo nextest run -p loopflow -j 4 --test task_pr_authority_tests --test-threads 4 -E 'test(valid_authority_publishes_and_records_the_pr)'
uv run python .lf/tmp/cut-i/run.py incident-cleanup cargo nextest run -p loopflow -j 4 --lib --test-threads 4 -E 'test(task_creation_refusal_preserves_inventory_and_marker_retry_reuses_provider_title) | test(task_creation_snapshot_failure_retries_without_starting_backlog) | test(task_preparation_rejects_unpublished_parent_before_allocating_child) | test(task_deletion_planning_) | test(task_worker_stop_) | test(provider_client_stop_preserves_unknown_process_evidence) | test(provider_client_stop_rechecks_identity_before_signaling)'
```

The additional post-create-failure and released-before-stop fixtures above are
still proposed work. Successful existing checks would preserve their narrower
contracts; they would not close either missing proof or establish installed
acceptance. Review found no reason to alter the accepted object model, add a
second ledger, or duplicate provider deletion. The concrete publication ordering
gap remains with main; process settlement retains its explicit deferred scope.

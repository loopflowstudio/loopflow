# Existing-Task stacking — unapplied contribution

2026-09-28 · LOO-298 · Jack authorized a bounded parallel contribution.
**Ready for main-worker integration; no executable edits or runtime proof.**
Only this note and [parallel-stacking.patch](parallel-stacking.patch) were written.
No Cargo, worker launch, Git mutation, installed Home access or provider write ran.

## Selected behavior

```sh
lf task prepare INF-123 --stack-on INF-124
# In INF-123's retained checkout:
lf rebase --plan
lf rebase
# Publish separately when authorized to update the existing GitHub PR's base.
```

An already-prepared root Task can select another Task's published PR. Selection
changes only `task_prs.parent_pr_id` and that PR row's `updated_at`. It preserves
Task/PR IDs, checkout and branch, the child's original `base_commit`, authored
commits/index/working files, publication and delivery evidence, Flow capture and
execution history. It neither rebases nor changes GitHub. `task run --stack-on`
uses the same preparation operation; it likewise does not imply integration.

Same-parent retry succeeds without a write. It resolves the retained parent PR,
not the parent Task's newer PR, if that Task has since rotated. Selecting a
different existing parent remains an explicit reparenting problem: this repair
covers the reported root-Task adoption and does not guess which old-parent
commits should be discarded. This scope choice is the contributor's judgment,
not an additional product decision attributed to Jack.

## Existing owners and concrete patch

- `ops/task.rs::prepare_task` invokes a small `stack_existing_task` operation
  after existing input validation. It resolves the requested Task and exact PR,
  and holds `lock_task_pr_mutation` through the narrow store write.
- `Store::stack_task_pr` / `SqliteStore::stack_task_pr` uses one immediate
  transaction. It compares the current child PR and original fork, checks the
  existing Work state and Flow claims, preserves same-parent retries, and
  rejects a pending merge request, unpublished/settled parent, cross-repository
  dependency or cycle. No migration, registry, lease or execution record is added.
- Generic `update_task_pr` compares `parent_pr_id` instead of rewriting it from
  a snapshot. Otherwise a stale publication/refresh could silently erase the
  selection. The one existing test that assigned the field directly now uses
  the supported operation.
- `ops/pr.rs::create_or_update_pr` rereads the base under its existing mutation
  lock before GitHub mutation. Its earlier base read can precede lengthy copy
  generation and a simultaneous parent selection. Existing publication identity
  and Task association remain with the current PR writer.
- `TaskPr` field documentation and CLI documentation distinguish selected parent
  from integrated Git base. The patch does not replace the rebase implementation.

`task_stack` already returns the child's recorded fork and the live parent branch
(or main after parent merge). `run_rebase` passes those into the existing owned
rebase/recovery path, preserves edits, and records the new base only after
success. `resolve_fork_point` already refuses an unreachable/non-ancestor fork;
changing the pointer never fabricates a replacement fork. Publication retargets
the existing PR through its existing update path when separately invoked.

## Coordination and unresolved inputs

Selection refuses an existing Flow claim without releasing it. Wait for normal
release or use the existing `stop_task_worker` recovery (exact PID/start identity
and claim comparison). The test's synthetic `release_flow` is not operational
permission to release a live worker. Selection and claim acquisition serialize
through existing SQLite transactions; a later claim sees the selected parent.

The PR lock covers selection/publication/head mutation, not an entire rebase.
Actual Git integration retains its existing sequencer owner and recovery rules.
A pending delivery request needs explicit delivery resolution first. Missing
publication or checkout needs ordinary publication/checkout recovery. Changed
parent, ambiguous identity, missing historical parent, rewritten fork or cycle
must remain explicit; do not guess replacement identity or discard commits.
Cross-repository stacking is not representable by the existing branch dependency.
Local publication permits selection; later provider reconciliation remains with
rebase/publication. No active LOO-298 claim or provider state was changed.

## Proposed proofs and actual checks

Proposed tests (not executed):

- `prepared_task_selects_parent_without_rewriting_work_or_publication`: real
  CLI/private Home, published child, committed/staged/unstaged/untracked bytes;
  selection, same-parent retry, failed selection and rejected stale publication.
- `stacking_preserves_claim_and_history_until_the_owner_releases`: rejects a
  held claim unchanged, releases the exact fixture claim, preserves capture and
  history through selection/retry, and rejects self-parenting/reverse-edge cycle.
- `existing_root_child_rebases_onto_parent_from_its_original_fork`: existing
  rebase API/local bare remote retains child content and integrates parent content.

Source review repaired the two publication races above and fixture errors around
initial PR publication and timestamp precision. Every proposed Rust file parsed
through `rustfmt` on stdin; final `git apply --check` passed. An unrelated H7
formatting hunk was removed; its first cleanup produced a malformed patch,
corrected before the final check. These are syntax/patch checks, not type checks
or behavioral passes. No executable file was written or formatted.

After integration, use the supervisor's bounded private-Home runner, resource
preflight, scrubbed LF/LOOPFLOW authority, nice +10, four Cargo workers and its
single build slot. Exact focused commands:

```sh
cargo nextest run -p loopflow -j 4 --lib --test flow_tests --test rebase_tests --test-threads 4 --no-fail-fast -E 'test(stacking_preserves_claim_and_history_until_the_owner_releases) | test(prepared_task_selects_parent_without_rewriting_work_or_publication) | test(existing_root_child_rebases_onto_parent_from_its_original_fork) | test(checkout_task_identity_ignores_main_and_parent_upstreams) | test(stacked_child_collapses_onto_main_dropping_squashed_parent) | test(stacked_rebase_refuses_when_base_is_not_an_ancestor)'
cargo nextest run -p loopflow -j 4 --test task_pr_authority_tests --test-threads 4 --no-fail-fast
cargo fmt --all --check
cargo clippy --all-targets -j 4 -- -D warnings
```

The authority suite covers the changed publication writer and retains
`valid_authority_publishes_and_records_the_pr` for the Task/PR association report.
Synthetic publication and a separate local rebase proof do not establish a full
CLI-to-GitHub demo or configured acceptance.

## Integration boundary

Patch built against dirty source at HEAD
`03330279f34a75480465a6c8239935c3ac4638ee`, with H7/shared edits present.
Main worker owns applying/adapting it. It references today's `flow_invocations`
claim owner; preserve the same transactional comparison if the Exec/FlowSession
conversion renames that owner. Keep the current `task prepare` spelling only
while it remains the actual CLI. Do not overwrite concurrent H7 changes or
copy old Project fixtures over its new flow/status schema.

Target paths: `ops/task.rs`, `ops/pr.rs`, `store/children.rs`,
`store/sqlite/children.rs`, `work/task/mod.rs`, `tests/flow_tests.rs`,
`tests/rebase_tests.rs` under `rust/loopflow/`, and `docs/lf.md`.

Source-path/bytes aggregate SHA-256: `9fca3014232db18b71ef01c444f308f41389853bcad46abaa64c8fac040acd97`.
Patch SHA-256: `4ed64e5784e5f807952792af79d538b712604d65646142f297f35c115b5b4f75`.

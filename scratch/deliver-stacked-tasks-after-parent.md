# Deliver stacked Tasks after parent merge (W2-93)

Ship a child PR that was stacked on unfinished parent work — after the parent
merges, squash-merges, is pruned, or diverges — with **zero manual git**. The
durable fix is to stop *inferring* the child's base at runtime and instead
*persist* it at placement, then make every consumer read that one record.

## The gap (why today breaks)

- `task_prs.base_commit` exists but is **always `origin/<default>`** — set at
  placement (`ops/task.rs:343`) and at serial rotation (`ops/task.rs:1571`).
  Serial PRs are siblings off main, never a real stack.
- **Rebase never reads it.** `plan_rebase` hardcodes `base_ref =
  origin/<default>` (`ops/rebase.rs:54`); `rebase_with_recovery` recomputes a
  fork point at runtime with `squash_merge_fork_point` → `git cherry`
  (`engine/git.rs:782`). `git cherry` matches *patch-ids*, so a squash merge
  (parent's N commits collapsed into one) is **not reliably detected**, and the
  child reintroduces parent changes or demands a hand `--onto`.
- **No parent relationship is stored anywhere.** No `parent_pr_id`,
  `stack_group_id`, or `stack_position` on any table (verified repo-wide). The
  only "parent" pointer is `TaskSession.project_session_id` (supervision).
- There is **no way to start a child before its parent merges**:
  `ensure_working_pr` only rotates once the prior PR `is_settled()`.

## User-visible outcome

A human or agent starts a child serial PR stacked on an *open* parent PR, keeps
committing, and later the parent merges/squashes/prunes/diverges. Landing the
child (or `lf rebase`) moves it onto `origin/<default>` carrying **only
child-authored commits** — no manual `git rebase --onto`, no reintroduced
parent commits. When ancestry is genuinely unsafe, the command **stops and
names the exact conflicting commits** instead of rewriting history.

## Source of truth

The `task_prs` **row** is the authoritative placement record. Two facts define a
stacked child; branch names stay hints only:

- **`base_commit`** (existing column, already treated as immutable identity by
  the optimistic-concurrency `WHERE base_commit=?` at
  `sqlite/child_sessions.rs:2853`) — redefined to the **real fork commit**: the
  parent PR's branch tip at fork time for a stacked child; `origin/<default>`
  tip for a root PR. It is an ancestor of the child's HEAD, so
  `git rebase --onto origin/<default> <base_commit>` replays exactly
  `base_commit..HEAD` — squash-proof, because it never relies on patch-id
  matching.
- **`parent_pr_id TEXT REFERENCES task_prs(id)`** (new, nullable) — the parent
  serial PR this child stacks on, or `NULL` when rooted on main. Drives
  lifecycle (detect parent merge/prune, clear on collapse). The parent's branch
  is read from its row as a *hint*, never as truth.

Everything else — `lf rebase --plan`, rebase execution, `lf pr` gh base, Task
status, PR range — is **derived** from these two fields. Audit history lives in
immutable `TaskEventKind` events; the row is mutated to reflect current truth.

## End-to-end proof

Integration tests (new `rust/loopflow/tests/stack_tests.rs`, temp bare origin +
clones like `test_rebase_efficiency.sh`) cover, for a child stacked on a parent:

1. **Merged parent** (normal merge) → child rebase drops parent commits; child
   diff intact.
2. **Squash-merged parent** → child rebase onto main omits the squashed parent
   commit; the case `git cherry` cannot catch today.
3. **Pruned parent** (parent branch deleted local + remote) → child still
   rebases via the persisted base sha; zero manual git.
4. **Divergent parent** (parent force-pushed to new history) → deterministic via
   the immutable base sha; child unaffected by the moved parent ref.
5. **Unsafe ancestry** (persisted base is not an ancestor of HEAD, e.g. child
   itself rewritten) → command **stops**, error names the commits between the
   last common ancestor and HEAD, no history rewrite.

Dogfood proof: on this Task's own worktree build a **3-PR stack** —
`lf pr stack --next b`, `lf pr stack --next c` while `a` is open — land `a`,
then `lf rebase --plan` on `b` prints the persisted fork base and `onto
origin/<default>`, `lf rebase` yields only `b`'s diff; repeat for `c`. Every
child lands with the expected diff and no manual git command.

## Affected surfaces and consumers

- **Schema/store**: new migration adding `task_prs.parent_pr_id`;
  `insert_task_pr`/`update_task_pr`/row hydration (`sqlite/child_sessions.rs`).
- **Model**: `TaskPr` (`task/mod.rs:186`) gains `parent_pr_id: Option<TaskPrId>`;
  `validate()` keeps `base_commit` required.
- **Placement/rotation** (`ops/task.rs`): base becomes the real fork point; new
  pre-merge stack path (below); reconcile clears `parent_pr_id` on parent merge.
- **Rebase** (`ops/rebase.rs`): plan carries the persisted fork base and prints
  it; executor feeds it as the `--onto` boundary (a `RebaseStrategy::RebaseOntoBase`
  / plan field `fork_base: Option<String>`) instead of the `git cherry`
  heuristic for Task PRs. Ancestry guard added.
- **PR surface** (`ops/pr.rs`): gh `--base` = parent branch when `parent_pr_id`
  set (GitHub auto-retargets to main on parent merge), else `<default>`; PR-body
  range uses the persisted base so it shows only child commits.
- **Land** (`ops/land.rs`): `rebase_land` target stays `origin/<default>` but
  routes through the persisted-base boundary.
- **Status/DTO**: Task status and `lf ls` (`lf/commands/waves.rs` base surface)
  show the chosen base + parent link; keep `--json` in lockstep (DTO no-defaults
  rule) with a fixture round-trip.
- **Runner prompt** (`task/runner.rs:1069`): document `lf pr stack --next`
  beside `lf pr land --next`.
- **Telemetry**: keep `.lf/metrics/ops.jsonl` recording the new class/strategy.

## Pre-merge stacking (the new capability)

`lf pr stack --next <slug>` is the open-parent counterpart to `lf pr land
--next`:

- `land --next`: settle parent (arm merge) → rotate child onto fresh
  `origin/<default>`. Unchanged.
- `stack --next`: **leave the parent PR open**, branch the child off the
  parent's current HEAD in the same worktree, persist `parent_pr_id` = parent
  and `base_commit` = parent tip. The runner owns rotation between PRs, same as
  today.

When the parent later merges (detected in `reconcile_task_pr`), the stack
collapses: null the child's `parent_pr_id`, emit an audit event, and the child's
next rebase moves onto main via the persisted base. After a successful
rebase-onto-main the child's `base_commit` is repointed to `origin/<default>`.

## Absent and error states

- **Base unreachable** (gc'd) → stop; name the missing sha and the exact
  `git rebase --onto` to run.
- **Base not an ancestor of HEAD** → stop; name the divergent commits; no
  rewrite. (`git merge-base --is-ancestor <base> HEAD` gate before any rebase.)
- **Parent abandoned while a live child stacks on it** → block the abandon (or
  require the child abandon/re-parent first): rebasing the child onto main would
  silently drop the parent's never-merged work.
- **Empty child** (no commits after base) → existing `Current`/`Noop` class.
- **Parent diverged** (force-push) → base sha is still an ancestor of the child;
  deterministic. The stale parent-branch hint is tolerated, not trusted.

## Operational boundary

The deterministic `--onto <base>` path is mechanical — it must **not** launch a
rebase agent; only genuine content conflicts escalate (preserves the
rebase-efficiency KR). No new network calls beyond the existing `fetch`.

## Ordered serial PRs

Persistence, the stack-placement command, and the deterministic land are
mutually dependent for *observable* behavior — the CLI `lf rebase`/`lf pr land`
paths are keyed on the worktree, not Task-aware, so a persisted base has no
effect until something both writes a stacked base and reads it back. So the
first landable slice folds them together; prune/divergence/full-surface
agreement follow.

1. **`stacked-land`** (this PR) — add `parent_pr_id` + migration and
   `TaskPr.parent_pr_id`; `lf pr stack --next <slug>` creates a child stacked on
   the open parent (base = parent tip, `parent_pr_id` set); `lf pr land` /
   `lf rebase` in a Task worktree resolve the active PR's persisted base
   (worktree → session → active PR) and rebase the child onto `origin/<default>`
   via `--onto <base>`, squash/merge-safe, with an ancestry guard that stops and
   names commits on unsafe history; reconcile clears `parent_pr_id` on
   parent-merge collapse (audit events preserved). Integration tests: merged and
   squash-merged parents → child diff only. Absorbs the earlier narrow "persist
   child base commit" diagnosis; ships the squash-proof core.
2. **`prune-divergence`** — pruned + divergent parent paths; parent-abandon
   guard; make `rebase --plan`, Task status, and PR range/gh base all surface
   the persisted base (full agreement + DTO/fixture); full
   merged/squash/pruned/divergent suite + the 3-PR dogfood.

## Exclusions

- **Cross-Task stacking** (Task B's worktree branched off Task A's branch). Each
  Task owns one worktree; "stack" here means the serial `TaskPr` chain within a
  Task. Out of scope.
- No change to the disposable-branch reset classes (`StaleEmpty`/`ScratchOnly`/
  `GeneratedOnly`) — they keep resetting to base.
- No generic multi-repo stack manager (wave bound: no platform ahead of need).

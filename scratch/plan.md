# LOO-1106: remove `gate`; `lf land` owns the clean pushed head

Stacked on LOO-1104 (PR #2047). LOO-1105 (PR #2046) stays separate. The
parent's plan is in its branch (`git show 26de21885e:scratch/plan.md`).

From the Task, word for word: "CI is the gate. Mechanical checks (`rust-lint`,
DTO/iOS parity, `migration-check`, `architecture-check`) stay in CI with
`land --wait-and-fix` and `ci-fix` owning the loop rather than running as a
separate local pre-gate runner or a generic `gate` LLM step on every PR.
Tailored reviews (`security-review`, `performance-review`) belong as dedicated
opt-in skills later, outside this Task."

## Outcome

- A Task on `code`, `feature` or `quick` goes from its last review step
  straight to `lf land --wait-and-fix`. No agent runs between them.
- `lf land`, `lf arm` and `lf pr submit` push one head, and that head has no
  branch-added file under `scratch/`, whether delivery skipped sync
  (`resync == false`), took `SyncStrategy::Noop`, merged cleanly, reused a
  recorded resolution, auto-dropped stacked-scratch conflicts, or handed real
  code conflicts to `sync-conflicts`. CI therefore runs the full matrix on the
  first head delivery pushes.
- During a real code conflict, `scratch/` notes stay on disk so `sync-conflicts`
  can read `scratch/plan.md`, and an aborted conflict leaves the branch
  untouched with no clear commit.
- A checkpoint PR from `pr-publish` keeps its notes, so CI stays deferred
  while `ship-review` is open. Nothing here changes `lf pr publish`.
- `lf gate`, `lf list graph`'s `queue` row and `lf skill customize gate` are
  gone.

## Demo

```text
$ lf graph show deliver
reflect
cmd: land --wait-and-fix

$ lf list graph | grep -E '^  (quick|deliver|queue) '
  deliver   reflect → cmd: land --wait-and-fix
  quick     loop(cmd: sync → implement) → qa → cmd: land --wait-and-fix

$ lf gate                 # fails as any unknown skill does; exit status non-zero
```

And the land itself, on a stacked child whose parent squash-merged (this
Task's own shape): `lf land` prints no "Launching sync agent to resolve
conflicts...", `git ls-tree -r --name-only origin/<branch> scratch/` prints
only `scratch/.gitkeep`, and the remote's `update` hook logged one push.

## Source of truth

- Graphs and skills: `rust/loopflow-definitions/src/builtins/**`; `build.rs`
  scans the directory, so deleting `gate.md` and `queue.yaml` unregisters them
  with no list to edit.
- "Branch-added scratch": `git::scratch::added(base, tree)`
  (`rust/loopflow-execution/src/git/scratch.rs`), unchanged. What changes is
  the base delivery hands it, and that delivery clears once on the head it is
  about to push.
- CI's own predicate, unchanged: `.github/workflows/ci.yml:88-117`
  (`find scratch -mindepth 1 ! -path 'scratch/.gitkeep'`).

## Findings behind the landing design

1. **The clear runs before the sync and never after.** `prepare_pr`
   (`ops/land.rs:170-181`) calls `scratch::clear`, then `sync_land`, which
   merges and pushes inside `finish_sync` (`ops/sync.rs:334-363`). On a
   conflict, `with_sync_retry` (`ops/sync_execution.rs:247-272`) runs
   `sync-conflicts`, `recover_sync` pushes whatever the resolver left, and the
   retry enters `prepare_pr` with `Integration::Completed`, which skips the
   clear (`land.rs:170`). `.github/CI.md:28-31` already documents the hole:
   "A sync can restore notes retained during conflict resolution after an
   earlier cleanup."

2. **A stacked child hits a modify/delete conflict on inherited notes.**
   `scratch::clear` measures against `task_diff_base` (`ops/scratch.rs:8-11`),
   which for a stacked Task is the parent's head, so the parent's notes are not
   "added". When the parent squash-lands without them, merging main via the
   synthetic comparison commit (`git/mod.rs:1236-1257`) stops with
   `CONFLICT (modify/delete): scratch/plan.md` (`git diff --diff-filter=U` and
   `git rerere remaining` both report `scratch/plan.md`). Every stacked child
   that edited an inherited note pays a `sync-conflicts` agent to land, and the
   resolver may keep the file.

3. **Clearing before sync breaks real conflict resolution and aborts.**
   Deleting `scratch/` in `prepare_pr` before `sync_land` removes
   `scratch/plan.md` from disk before `sync-conflicts` runs (despite
   `sync-conflicts.md:9` instructing the resolver to read `scratch/`), and
   leaves a `"Clear added scratch files"` commit and deleted notes behind if
   the conflict aborts.

4. **`SyncStrategy::Noop` skips the push during delivery.**
   `start_owned_sync` (`ops/sync.rs:291-293`) hardcodes `push = false` for
   `SyncStrategy::Noop`. When a persistent worktree or an already-integrated
   stacked Task enters `sync_land` (`plan_sync` lines 142-144 pick `Noop`),
   delivery never pushes the cleared (or newly committed) head.

5. **Three smaller boundary leaks on the same delivery path:**
   - `sync_land` (`ops/land.rs:406`) hardcodes `push: true` even when
     `options.local` is true.
   - On `prepare_pr`'s `resync == false` path, `require_task_pr_range_nonempty`
     is a no-op for non-Task branches, so clearing a notes-only non-Task PR
     could push an empty tree.
   - The standalone already-armed shortcut (`ops/land.rs:97-113`) returns early
     without checking whether `HEAD` still carries branch-added `scratch/`
     files (a manually armed checkpoint PR would be evicted by the merge queue).

6. **A PR head with notes is not a failing head.** `tests-result` is skipped
   for a checkpoint (`ci.yml:449`), GitHub counts a skipped required check as
   satisfied, and `scratch-clear` fails only on `merge_group`. Auto-merge
   would queue it, the queue would evict it, and `ci-fix` (an agent) would
   repair what a `git rm` fixes. `pr_landing` needs no new case for this once
   delivery cannot push such a head: `ci-fix` re-arms through `lf arm`, which
   is `prepare_pr`.

7. **`finish_sync` is the one place every delivery sync ends**: clean merge,
   `Noop`, `rerere` reuse (`sync.rs:308-311`), auto-dropped scratch conflicts,
   and agent recovery (`recover_sync`, `sync.rs:245-254`) all call it with the
   `SyncExpectation` built at the start, and only delivery sets it
   (`sync_for_delivery`, `sync.rs:210-216`).

## The landing design

One rule: **delivery clears branch-added `scratch/` files once, on the head it
is about to push, measured against the integration target (`target_sha` after
sync, or `merge_base(HEAD, integration_base)` when no sync runs).**

- **`ops::scratch::clear(repo, base)`** takes its base explicitly.
  `lf scratch clear` passes today's base (`task_diff_base`, else
  `git::scratch::base`), so a stacked child with a live parent keeps the
  parent's notes out of its diff.
- **`SyncExpectation.require_changes` becomes `delivery: bool`**: one field
  naming the mode (`sync_for_delivery` sets `true`; plain `lf sync` sets
  `false` and keeps notes untouched).
- **When `resync == true` (`sync_land` runs):**
  - `prepare_pr` does **not** clear `scratch/` before `sync_land`. Notes stay
    on disk through the merge and any `sync-conflicts` session.
  - `sync_land` passes `push: !options.local` to `sync_for_delivery`.
  - In `start_owned_sync`:
    - `SyncStrategy::Noop`: passes `options.push && expected.delivery` (instead
      of hardcoded `false`) to `finish_sync`, so delivery still clears scratch,
      checks non-empty, and pushes when remote differs from `HEAD`.
    - `SyncStrategy::MergeTarget` conflict path (`intervention_state == Some("merge")`):
      after `git::merge`, read `remaining = rerere_remaining(repo)?`. When
      `expected.delivery && !remaining.is_empty()`, compute
      `merge_base = git::merge_base(repo, "HEAD", &target_sha)?` and
      `added = git::scratch::added(repo, &merge_base, "HEAD")?`. If **every**
      path in `remaining` is in `added` and absent from `target_sha`
      (`git cat-file -e <target_sha>:<path>` fails), remove those paths with
      `git --literal-pathspecs rm -f -- <remaining>` and call
      `continue_merge(repo, None)`. That resolves both pure stacked-scratch
      `UD` conflicts and `rerere`-resolved code + scratch conflicts with zero
      agent launches, while preserving renamed repository documents (not in
      `added`) and leaving `scratch/` intact on disk whenever a real code
      conflict goes to `sync-conflicts`.
  - In `finish_sync`:
    1. Verify integrated head (`verify_integrated_head`) and zero unresolved
       paths (`git diff --diff-filter=U --name-only` empty) first, so cleanup
       never mutates a broken merge or wrong branch.
    2. When `expected.delivery`: run `ops::scratch::clear(repo, target_sha)` to
       remove any remaining branch-added `scratch/` files (including anything
       `sync-conflicts` kept or wrote).
    3. When `expected.delivery`: refuse if
       `HEAD^{tree} == target_sha^{tree}` ("no authored changes remain after
       integration; refusing to publish an empty PR") before any push.
    4. Read the post-clear `HEAD` into `SyncVerification`, validate Task PR
       range (`!expected.stacked`), push once if `push` (skipping a no-op push
       if `origin/<branch>` already matches `verification.head`), and run
       `record_control_task_state` (which already records `target_sha` and
       clears `parent_pr_id` on stacked Tasks).
  - Because `finish_sync` always records the stack base from `target_sha` and
    the clear commit may sit on top of the merge commit, delete both
    `accept_completed_integration`'s `HEAD^2` read (`land.rs:419-427`) and
    `sync_land`'s redundant second `record_stack_sync` (`land.rs:411-415`).
- **When `resync == false` (`prepare_pr` pushes without syncing):**
  - `prepare_pr` clears `scratch/` against `base = git::scratch::base(&repo_root)?`
    (`merge_base(HEAD, integration_base)`), checks non-empty for both Task
    (`require_task_pr_range_nonempty`) and non-Task
    (`HEAD^{tree} != base^{tree}` -> same refusal before push), and calls
    `push_with_upstream_if_needed`.
- **Standalone already-armed shortcut (`land.rs:97-113`):**
  - Only return early when `git::scratch::added(&repo_root, &git::scratch::base(&repo_root)?, "HEAD")?.is_empty()`, so a manually armed checkpoint PR is cleared and re-pushed instead of evicted by the merge queue.
- **PR copy stays before integration (`land.rs:156-169`):**
  - `published_pr_copy` must compare `HEAD` against GitHub's `headRefOid`
    before pushing so a local commit generates fresh copy while an unchanged
    published PR reuses its copy without an LLM call.
- **`Integration::Completed`** keeps refusing a dirty tree and adds nothing:
  the head it receives was cleared and pushed by `finish_sync`.
  `release.rs:1701` calls `finish_arm_after_sync` with no sync before it and is
  unaffected.

Passed over: clearing in `prepare_pr` before every sync plus a second clear in
`finish_sync` (deletes `scratch/plan.md` before `sync-conflicts` can read it
and leaves a clear commit on aborted conflicts); clearing in `prepare_pr` after
sync returns (needs sync not to push, duplicates push/force-with-lease logic,
and misses `recover_sync`'s push); teaching `sync-conflicts` to delete notes
(hole stays open to any resolver); a `pr_landing` repair for `scratch-clear`
(unreachable once delivery cannot push notes).

Sibling variants: `arm`, `submit`, `finish_arm_after_sync`,
`finish_submit_after_sync` all go through `prepare_pr`; `lf land --local`
syncs through `sync_land` with `push: false` and gets the same clear with no
push. Plain `lf sync` (`delivery: false`) keeps notes, as today.

## Absent and error states

- No `scratch/` files added: no clear commit, head unchanged, an armed exact
  head stays armed (`land.rs:83-113` returns before any of this).
- Existing scratch content the target has (edited or renamed into `scratch/`
  on the branch): kept, as `land_cleans_added_scratch_and_retains_existing_edits`
  and `clear_preserves_existing_files_moved_into_scratch` assert.
- Real code conflict aborted (`lf sync --abort`): zero `"Clear added scratch
  files"` commit created, all `scratch/` notes intact.
- Clearing leaves no authored change (Task or non-Task, `resync` true or
  false): refused before any push, as
  `land_refuses_when_clearing_scratch_leaves_no_authored_change` asserts.
- A repository without CI's `scratch-clear` job is unaffected; `.github/CI.md`
  keeps "repositories retaining it must define their own candidate policy".
- A repo skill or Graph that still names `gate` (`extends: builtin`, or a
  `- gate` step) fails to load with the existing unknown-skill error. This
  repository's own are fixed here; others are a release note.

## What goes

Skills and Graphs:
- `rust/loopflow-definitions/src/builtins/task/skill/gate.md`, `.lf/skills/gate.md`.
- `deliver.yaml`: `reflect`, `cmd: land --wait-and-fix`; description "Learn
  from what people said, then land the branch and stay until it merges."
- `quick.yaml`: drop `- gate`.
- `queue.yaml`: deleted. Without `gate` it is `compress` + `refresh`; no
  Graph, Wave config or doc command uses it. With it,
  `.lf/graphs/finish-queue.yaml` (unreferenced, its only caller).
- `.lf/graphs/pursue-auto.yaml`: drop `gate` and the outer loop it decided
  (`loop-or-next` after it); the two inner loops stay.

The cached PR copy, gate's old handoff, which nothing writes any more
(`git grep pr-title.txt` finds only readers and tests):
`consume_gate_artifacts`, `read_cached_pr_copy` and the `.pr-copy-ref` rules
in `ops/pr.rs:120-123,403-470`; the cached branch of `resolve_pr_copy`
(`land.rs:334-338`); tests `land_uses_cached_pr_copy_when_available`,
`land_generates_copy_when_cached_pr_copy_is_stale` (`land_tests.rs:906,1576`),
`gate_artifacts_never_reach_the_published_head` (`work/pr_tests.rs:608`);
`docs/dev/delivery.md:140`. `published_pr_copy` (same-head reuse) and
generation stay.

Prose that names the skill (each line rewritten to say CI, or deleted):
- Builtin skills: `compress.md:29`, `implement.md:71,77`, `kickoff.md:54`
  ("Acceptance check at gate" becomes "Acceptance check"), `verify.md:25,27`
  (the fast path cites the `qa` record and the CI run; a diff outside
  `scratch/` from `land` or a sync still means the full check),
  `task_operate.md:62`, `ship-review.md:8`, `sync-conflicts.md:27`,
  `wave_operate.md:68`. Golden: `tests/goldens/builtin_implement.md`
  (`uv run python tests/goldens/update_goldens.py`).
- Engine strings: `graph/output.rs:19`, `graph/instructions.rs:73`.
- `AGENTS.md:186,448,454-456`: line 186 drops `gate:` from the focused test
  example; line 448 becomes land's ("**land:** clears scratch and pushes once;
  CI runs the suites, `ci-fix` repairs"); lines 454-456 align with
  `pr-message.md:25` and `pr-land.md` — checks that can't run locally go to CI,
  CI shows automated test/lint results, and only post-merge checks and
  unrecoverable facts move into the PR body before landing.
- `.github/CI.md:5-31`: delivery clears on the pushed head; the manual
  "after the final `lf sync`, run `lf scratch clear`" procedure and the
  "sync can restore notes" warning go. Lines 115-116 (`.lf/tmp/gate`, the
  test runner) stay.
- `scripts/testing.md:3,35-42,85,139-140,164-166,228,232`; `scripts/test.py:5`.
- Docs: `README.md:71`, `docs/index.md:71,157,188-190`,
  `docs/getting-started.md:54-55,67,134`, `docs/ops.md:19,60-85` (the "Review
  while CI runs" section goes), `docs/orchestration.md:14,22,30,89,120,232,247,254,261`
  (the customize example moves to another skill), `docs/config.md:101`,
  `docs/dev/planning.md:551`, `docs/dev/index.md:360` + `index.html:1873`,
  `docs/troubleshooting.md:139`, `docs/dev/delivery.md:326,358`,
  `docs/dev/reference-runtime.md:255` (both name a `ship` Graph that does not
  exist; the sentences go), `docs/surfaces.md:294` (`queue` row),
  `swift/README.md:1276,1460,1494`,
  `rust/loopflow/tests/README.md:265,439,552,571,585`,
  `tests/e2e/README.md:167`, `.lf/qa/desktop.md:45`, `website/README.md:50`,
  `website/home.html:20,88`, `website/main.py:302`,
  `design/prototypes/homepage-next/index.html:1879`.

Tests that load the builtin (repointed at a surviving skill such as `qa`, or
their expected lists shortened):
- `graph/compiled.rs:599,626,629` (drop the `queue` row; `quick` and
  `deliver` lose `gate`), and `:369-372`.
- `builtins.rs:285,325`; `graph/mod.rs:1367-1371` and `:2490`
  (`deliver_graph_gates_then_lands_and_waits`: two items, renamed);
  `lf/commands/graph.rs:1646` (drop `queue`); `task_graph.rs:648`;
  `skills/catalog.rs:454-499`.
- DTO fixtures naming a `code` description "design, implement, gate, land"
  (`rust/loopflow-execution/tests/fixtures/dto/server/skill_entries.json`,
  `tests/fixtures/dto/server_schema.json:7585`) lose the word.

Stays: fixtures that use `gate` or `queue` as an arbitrary step, Graph or
commit-prefix label (`task_updates.rs`, `graph_inventory.rs`,
`task_create_tests.rs`, `task_graph_launch_tests.rs`, DTO step lists, Swift
tests); the test runner's "gate" (`scripts/test.py`,
`python/tests/test_gate_bounded.py`, `.lf/tmp/gate`); installation entry
gate, release gate, merge gate (`MergeGateReading`), `UI_HOST_GATE`; release
notes and applied migrations; `.lf/wave/*/MEMORY.md` (read-only in a Task
checkout).

Must survive: `lf scratch clear` as a standalone command with today's base;
`lf pr publish` keeping notes; one push per land
(`land_preserves_checkpoint_history_and_pushes_the_final_tree_once`);
exact-head re-arm without a push; existing and moved-in scratch content.

## Costs of dropping the step

- Nobody reads the diff for security, leftovers and stale docs between
  `ship-review` and merge. `compress`, `concept-review`, `qa` and
  `ship-review` come before; `quick` has only `qa`. Tailored reviews are a
  later opt-in skill.
- No agent picks "two to five tests" or confirms CI started for the head.
  `land --wait-and-fix` reads the head's checks (`decide_merge`,
  `pr_landing.rs:175-263`) and holds until merge.
- Nothing copies check results from `scratch/` into the PR body by hand; CI
  shows automated checks and `pr-message` keeps only post-merge checks.

## Choices made for review

1. **Single-owner delivery clear (no pre-sync clear when `resync == true`).**
   Delivery clears scratch once, right before pushing (`finish_sync` when
   syncing, `prepare_pr` when `resync == false`), and auto-drops pure
   stacked-scratch `UD` merge conflicts inside `start_owned_sync` so
   `scratch/plan.md` stays on disk for `sync-conflicts` on real code conflicts
   and an aborted conflict leaves the branch untouched. Passed over: clearing
   in `prepare_pr` before sync plus a second clear in `finish_sync`.
2. **`queue` and `finish-queue` are deleted**, not kept as `compress` +
   `refresh`. Passed over: keeping `queue` as a two-step Graph nobody runs.
3. **The cached PR copy code is deleted here**, as gate's leftover. Passed
   over: leaving it for a later cleanup.
4. **`pursue-auto` loses its outer loop** with `gate`. Passed over: keeping a
   loop whose only step is another loop plus `loop-or-next`.
5. **`lf scratch clear` stays as a standalone CLI command** with today's base
   (`task_diff_base` else `git::scratch::base`), useful for triggering CI on a
   manual `lf pr publish` without arming auto-merge. Passed over: deleting the
   CLI subcommand.

## Acceptance check

```bash
cargo build -p loopflow
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') sh -c '
  lf="$PWD/target/debug/lf"; tmp=$(mktemp -d) && git -C "$tmp" init -q && cd "$tmp" &&
  "$lf" graph show deliver && "$lf" graph show quick &&
  ! "$lf" list graph | grep -E "^  queue " && ! "$lf" help gate' < /dev/null
git grep -nIwi gate -- rust/loopflow-definitions/src/builtins .lf/skills .lf/graphs   # no output
uv run python scripts/test.py --reuse-passing --rust-filter \
  'test(graph::) | test(builtins::) | test(skills::catalog::) | binary(golden_prompt) | test(documented_commands::) | test(land_tests::) | test(scratch_tests::) | test(pr_tests::)'
```

Expected: `deliver` prints `reflect`, `cmd: land --wait-and-fix`; `quick`
prints `cmd: sync`, `implement`, `qa`, `cmd: land --wait-and-fix`; the grep is
empty; the tests pass, including the new Pass 1 tests below. CI runs the suites.

## Passes

1. **Done (`42aa4e92d9`):** Landing single-owner scratch clear (`ops::scratch::clear`
   takes `base`, `SyncExpectation.delivery`, `Noop` delivery push, mechanical
   branch-added scratch conflict drop in `start_owned_sync`, `finish_sync`
   clear before empty-tree check and single push, `sync_land` `push: !options.local`
   + `finalize_local` ancestor-checked remote branch deletion,
   `matching_task_pr_merge_request` returning `Ok(None)` for unpublished Tasks).
   Verified with all 3 new `land_tests` scenarios (conflict resolution for
   `submit` & `land`, stacked child without agent launch, `Noop` delivery) plus
   existing `land_tests` and `scratch_tests`.
2. **Done (`8294357459`):** Removed builtin `gate.md`, `.lf/skills/gate.md`,
   `queue.yaml`, `.lf/graphs/finish-queue.yaml`, `gate` steps from `deliver`,
   `quick`, `pursue-auto`, dead cached PR copy handoff, and updated builtin
   skill prose, engine instructions, DTO fixtures, tests, and `builtin_implement.md`
   golden. Added **Trust & data boundary** and focused-review prompts to
   builtin `kickoff.md` and `ship-review.md` plus Loopflow's concrete triggers
   in `.lf/skills/kickoff.md` and `.lf/skills/ship-review.md`.
3. **Done (`a6c63e56c5`):** Updated internal/developer docs (`AGENTS.md`,
   `docs/dev/*`, `scripts/testing.md`, `scripts/test.py`, README guides,
   prototype HTML) and regenerated `docs/dev/index.html`.

Nothing remains to build. On merged head `95c5cb1357` (2026-11-25):
the acceptance `git grep` is empty, `deliver`/`quick` read as the Demo shows,
`gate.md`/`queue.yaml` are gone. `scripts/test.py` stopped at its resource
preflight (`resource_envelope.py --recover` timed out after 300s), so the
acceptance filter ran through `cargo nextest` directly: 191 passed, 2 failed —
`pr_tests::task_snapshot_reads_its_current_parent_project` and
`pr_tests::task_pr_missing_cached_linear_url_refuses_before_remote_mutation`,
both panicking at `loopflow-test-support/src/lib.rs:483` ("retained fixture
input: QueryReturnedNoRows", a `tracker_items` Linear fixture row). This branch
does not touch those tests, the fixture, or tracker code; CI decides whether
they are pre-existing or Machine-local.

## User docs written (`write-docs`)

Already updated in place to describe the shipped behavior:
- `README.md`
- `docs/index.md`
- `docs/getting-started.md`
- `docs/ops.md`
- `docs/orchestration.md`
- `docs/config.md`
- `docs/troubleshooting.md`
- `docs/surfaces.md`
- `.github/CI.md`
- `website/home.html`
- `website/main.py`

Done when `lf graph show deliver` and `lf graph show quick` print no `gate`;
`lf gate` and the `queue` Graph are unknown; a land through a conflicting
sync, a stacked child's land, and a `Noop` delivery each push one head with no
branch-added `scratch/` file while keeping notes readable during real code
conflicts; and the acceptance check passes.

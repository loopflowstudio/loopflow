# LOO-1106: remove `gate`; `lf land` clears `scratch/` on the head it pushes

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
  branch-added file under `scratch/`, whether the sync merged cleanly, reused
  a recorded resolution, or handed conflicts to `sync-conflicts`. CI therefore
  runs the full matrix on the first head delivery pushes.
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
  the base delivery hands it, and when.
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

2. **A stacked child always takes that path.** `scratch::clear` measures
   against `task_diff_base` (`ops/scratch.rs:8-11`), which for a stacked Task
   is the parent's head, so the parent's notes are not "added" and survive
   the clear. The stacked merge then compares against a synthetic commit of
   main's tree on the fork base (`git/mod.rs:1236-1257`). Probe, temp repo,
   parent adds `scratch/plan.md` + `scratch/qa.html`, child edits `plan.md`,
   parent squash-lands without them:

   ```text
   added vs merge-base(HEAD, main): child.txt parent.txt scratch/plan.md scratch/qa.html
   added vs parent head:            child.txt
   CONFLICT (modify/delete): scratch/plan.md deleted in <cmp> and modified in HEAD.
   UD scratch/plan.md
   D  scratch/qa.html
   ```

   So every stacked child that touched an inherited note pays a
   `sync-conflicts` agent to land, and the resolver may keep the file.

3. **A PR head with notes is not a failing head.** `tests-result` is skipped
   for a checkpoint (`ci.yml:449`), GitHub counts a skipped required check as
   satisfied, and `scratch-clear` fails only on `merge_group`. Auto-merge
   would queue it, the queue would evict it, and `ci-fix` (an agent) would
   repair what a `git rm` fixes. `pr_landing` needs no new case for this once
   delivery cannot push such a head: `ci-fix` re-arms through `lf arm`, which
   is `prepare_pr`.

4. **`finish_sync` is the one place every delivery sync ends**: clean merge,
   rerere reuse (`sync.rs:308-311`) and agent recovery (`recover_sync`,
   `sync.rs:245-254`) all call it with the `SyncExpectation` built at the
   start, and only delivery sets `require_changes` (`sync_for_delivery`,
   `sync.rs:210-216`).

## The landing design

One rule: **delivery clears scratch files the integration target lacks, on
the head it is about to push, and pushes after.**

- `ops::scratch::clear(repo, base)` takes its base. `lf scratch clear` passes
  what it passes today (`task_diff_base`, else `git::scratch::base`), so a
  stacked child with a live parent keeps the parent's notes out of its diff.
- Delivery passes `merge_base(HEAD, <integration target>)`:
  - before the sync (`land.rs:171`), which removes inherited notes so finding
    2's conflict never forms, and covers the no-resync path (a CI repair on a
    mergeable PR, `needs_resync` false) where no sync runs;
  - inside `finish_sync`, when the expectation is a delivery, before
    `verify_sync` and the push. After a merge the base is the pinned
    `target_sha`, so the same call removes anything a resolver restored or
    wrote. `require_changes` then also sees a notes-only branch as empty.
- The clear commit sits on top of the merge commit, so
  `accept_completed_integration`'s `HEAD^2` read (`land.rs:419-427`) goes.
  `finish_sync` already records the stack base from the pinned target
  (`record_control_task_state`, `sync.rs:441-468`); pass 1's stacked test
  shows whether `sync_land`'s second `record_stack_sync` (`land.rs:411-415`)
  is redundant too, and deletes it if so.
- `SyncExpectation.require_changes` becomes the name for what it now means
  (delivery), one field, not a second flag beside it.
- PR copy is still resolved before the first clear (`land.rs:156-169`), so a
  generated body can read the notes.
- `Integration::Completed` keeps refusing a dirty tree and adds nothing: the
  head it receives was cleared and pushed by `finish_sync`.
  `release.rs:1701` calls `finish_arm_after_sync` with no sync before it and
  is unaffected.

Passed over: clearing in `prepare_pr` after the sync returns (needs the sync
not to push, then a second push site with sync's force-with-lease rules, and
still misses the push `recover_sync` makes); teaching `sync-conflicts` to
delete notes (the hole stays open to any resolver); a `pr_landing` repair for
a `scratch-clear` failure (unreachable once the push is clean).

Sibling variants: `arm`, `submit`, `finish_arm_after_sync`,
`finish_submit_after_sync` all go through `prepare_pr`; `lf land --local`
syncs through the same `sync_land` and gets the same clear with no push.
Plain `lf sync` (not delivery) keeps notes, as today.

## Absent and error states

- No `scratch/` files added: no clear commit, head unchanged, an armed exact
  head stays armed (`land.rs:83-113` returns before any of this).
- Existing scratch content the target has (edited or renamed on the branch):
  kept, as `land_cleans_added_scratch_and_retains_existing_edits` asserts.
- Clearing leaves no authored change: refused before any push, as
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
- `AGENTS.md:186,448,454,456`: the cadence line becomes land's ("**land:**
  clears scratch and pushes once; CI runs the suites, `ci-fix` repairs"), and
  the check lines move to the PR body when `lf land` writes it.
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

Must survive: `lf scratch clear` as a command with today's base;
`lf pr publish` keeping notes; one push per land
(`land_preserves_checkpoint_history_and_pushes_the_final_tree_once`);
exact-head re-arm without a push; existing scratch content.

## Costs of dropping the step

- Nobody reads the diff for security, leftovers and stale docs between
  `ship-review` and merge. `compress`, `concept-review`, `qa` and
  `ship-review` come before; `quick` has only `qa`. Tailored reviews are a
  later opt-in skill.
- No agent picks "two to five tests" or confirms CI started for the head.
  `land --wait-and-fix` reads the head's checks (`decide_merge`,
  `pr_landing.rs:175-263`) and holds until merge.
- Nothing copies check results from `scratch/` into the PR body by hand;
  generated PR copy reads the notes before they are cleared.

## Choices made for review

1. **`queue` and `finish-queue` are deleted**, not kept as `compress` +
   `refresh`. Passed over: keeping `queue` as a two-step Graph nobody runs.
2. **The cached PR copy code is deleted here**, as gate's leftover. Passed
   over: leaving it for a later cleanup.
3. **The post-sync clear lives in `finish_sync`**, so sync owns the pushed
   head for delivery. Passed over: a second push site in `land.rs`.
4. **`pursue-auto` loses its outer loop** with `gate`. Passed over: keeping a
   loop whose only step is another loop plus `loop-or-next`.

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
empty; the tests pass, including the two new ones below. CI runs the suites.

## Passes

1. **Landing, the pass that could prove the plan wrong.** `scratch::clear`
   takes a base; delivery clears before the sync and in `finish_sync`; the
   `HEAD^2` read goes. New tests in `rust/loopflow/tests/land_tests.rs`, each
   with the remote `update` hook counting pushes:
   - a conflicting sync whose resolver (the fixture's `agent_script`) resolves
     the conflict and writes `scratch/notes.md` back: the remote head has no
     `scratch/notes.md`, one push, `lf pr submit` and `lf land` both;
   - a stacked child that edited an inherited `scratch/plan.md`, parent
     squash-landed: no agent launched, remote head has only `.gitkeep`, the
     recorded stack base is the target, one push.
   It shows whether a commit on top of the merge survives `verify_sync`, the
   stack record and the Task PR range checks. If it does not, the plan is
   wrong here and realign takes it.
2. **Remove `gate`.** The skill, overlay, Graph steps, `queue`,
   `finish-queue`, cached PR copy, builtin skill prose, engine strings, test
   expectations and the golden.
3. **Docs.** Everything under "Prose that names the skill" outside the
   builtins, plus `.github/CI.md` and `AGENTS.md` describing the land that
   pass 1 built.

Done when `lf graph show deliver` and `lf graph show quick` print no `gate`;
`lf gate` and the `queue` Graph are unknown; a land through a conflicting
sync and a stacked child's land each push one head with no branch-added
`scratch/` file; and the acceptance check passes.

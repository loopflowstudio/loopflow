# LOO-885: gate starts CI first, then reviews

## Problem

Gate reviews the diff and runs the affected local suites before anything is
published, so hosted CI starts last and repeats tests gate already ran. In
Loopflow it is worse: a PR with branch files in `scratch/` is a checkpoint and
CI skips its suites, so the first real run happens when `lf land` clears
scratch after gate has finished.

Measured 2026-11-13:

- Checkpoint PRs #1916 and #1910 (scratch files present): `scratch-clear` and
  `macos-inputs` pass, the other 15 jobs are `skipping`, `gh pr checks` exits 0
  and the run concludes `success` in 12–19 s.
- Candidate PR #1915 (scratch clear): the matrix ran 14–15 min per head
  (16:28:13 → 16:42:33, 15:53:43 → 16:08:57).

So a "passing" check list can mean nothing ran, and a gate that starts CI at
entry gets a full matrix result inside the time a review takes.

## Demo

`lf ship LOO-885` (or `lf gate` in this checkout). Within the gate's first
minutes the PR's checks list `rust-test`, `rust-lint`, `python-test` queued or
running for the head gate just pushed, while gate is still reading the diff.
Gate's report ends with one line per head: what CI and the few local tests
showed for that SHA, and what is still pending or never ran. No auto-merge is
requested until the `land` step.

This Task's own deliver step runs the branch's `.lf/skills/gate.md`, so its
gate is the first real run of the new sequence.

## Approach

Gate becomes four moves, in both the builtin and Loopflow's copy:

1. **Prepare and start CI.** Read the diff summary, the repo guide and the
   notes, far enough to publish one coherent candidate: commit open work, run
   the formatter's auto-fix, follow any step the repository's guide names for
   making a PR one CI tests, `lf sync`, `lf pr publish`. Nothing is armed.
2. **Confirm it started.** The PR's `headRefOid` equals local `HEAD`, and the
   real suites are queued or running for it. Suites reported `skipping`, or
   only a classifier job passing, mean CI did not start: fix the cause or
   report it. That is never a pass.
3. **Review while it runs.** Security and data handling where the change
   touches them, correctness, failure handling and recovery, ownership,
   interfaces, maintainability, docs. Fix concrete findings in scope. Name the
   two to five tests that cover the riskiest or most-edited behavior and say
   why in a line each; the design's acceptance check is one of them. CI runs
   them along with everything else. Run one locally only for fast feedback on
   a fix, to reproduce a CI failure, or to probe a risk no suite covers.
4. **Publish fixes and report by head.** Batch fixes into one publish per
   review pass (each push cancels the previous CI run). Read checks for the
   final head. Report passing, failing, pending, skipped and unavailable
   separately; green on an earlier head says nothing about later edits. Gate
   does not wait for a pending run: `land --wait-and-fix` owns that.

### Where the Loopflow part lives

The builtin says only "the repository's guide may name a step that makes a PR
one CI tests; do it before publishing". `.lf/skills/gate.md` holds Loopflow's
steps and note handling, once; `.github/CI.md` (owner of the `scratch-clear`
classifier) links to it:

```bash
lf commit -m "gate: checkpoint"     # notes are in history at this SHA
lf sync
lf scratch clear                    # branch-added scratch only; base files stay
lf pr publish
lf pr checks                        # suites queued or running, not skipping
```

Working context after the clear:

- The plan and notes were loaded into gate's conversation at launch and stay
  readable with `git show <checkpoint-sha>:scratch/<file>`.
- What a later reader needs (what people said, what a run showed, checks that
  only run after merge) goes into the PR body before the clear, as pr-submit
  already requires before its own clear.
- Gate writes nothing new under `scratch/`: `lf commit` and `lf pr publish`
  stage it, the next head becomes a checkpoint and CI skips again. Findings
  that are fixed need no note; the rest goes in the report and the PR body.
  The cached PR copy files (`scratch/pr-title.txt`, `pr-body.md`,
  `.pr-copy-ref`) are the exception: `consume_gate_artifacts`
  (`rust/loopflow/src/ops/pr.rs`) deletes them before publish commits,
  except in a persistent worktree (`is_persistent_worktree`), where gate
  passes `--title`/`--body` instead.
- A gate that ends with work it cannot finish restores the notes with
  `git revert --no-edit <clear-commit>` and says so. The head is a checkpoint
  again, which is the truth.

`lf land` after gate finds nothing to clear (`ops::scratch::clear` returns 0).

### Fallbacks

- **No PR yet** (`queue` graph, bare `lf gate`): `lf pr publish` creates a
  ready PR. Assumption recorded in `scratch/questions.md`.
- **No remote, no `gh`, publish refused, or no CI configured:** say which, run
  the selected tests locally as the only automated validation, and report "no
  CI ran". Local green is reported as local.
- **CI configured but the relevant suites don't trigger** (path filter, label,
  fork): report the suites that did not run by name.

## Implementation state (2026-11-13)

Both gate skills use prepare/publish, confirm the head and real suites,
review while CI runs, then publish fixes and report by head. The builtin has
no scratch cleanup requirement; Loopflow's copy owns candidate cleanup and
context retention through checkpoint history and PR copy. Sibling skills and
user docs describe focused local feedback with CI owning suites.

`ops::scratch::clear` and the CI classifier are unchanged. `lf pr publish`
creates/refreshes a ready PR without arming. Cached PR copy is read and
removed by `consume_gate_artifacts` before publication commits; its ref must
name HEAD or its immediate ancestor, so prepare those files after cleanup.

Remaining: packaging and live gate acceptance below.
Publication and the live matrix observation belong to delivery, not implement.
Realign 2026-11-13: branch is level with origin/main; no PR exists yet, so
the first `lf pr publish` in gate creates it.

## Alternatives

- **Run the matrix on checkpoint PRs** (drop or relax the classifier, or key it
  on draft/ready or a label). Starts CI even earlier and needs no cleanup, but
  reverses LOO-343: every checkpoint push would run a 15-minute matrix plus
  hosted macOS. Rejected; gate entry is where a checkpoint becomes a
  candidate.
- **A command step before gate** (`deliver: [cmd: pr submit, gate, land]`).
  Deterministic and earlier, but gate would launch with scratch already
  cleared and no plan in context, bare `lf gate` and `queue` would not get it,
  and submit marks the Task as waiting for a merge click. Rejected.
- **Keep scratch on disk but untracked** (clear, restore the files, ignore them
  through `.git/info/exclude`). Later steps would still load the plan, but it
  adds a second state for scratch, a new command mode, and stale notes for the
  next Task in the checkout. Git history already keeps them. Rejected.
- **Delete `.lf/skills/gate.md` and put Loopflow's part in AGENTS.md.** One
  copy fewer, but the builtin cannot preload `scripts/testing.md`. Kept as a
  copy; out of scope here.

## Scope

Prose only; no Rust or workflow change.

- `rust/loopflow/src/builtins/task/skill/gate.md`: rewrite around the four
  moves. Frontmatter `produces` becomes the published candidate, review fixes
  and a readiness report by head. Keep Scope and Adaptation. Keep cleanup,
  README/docs and inline-comment review as review items, shorter.
- `.lf/skills/gate.md`: same sequence plus the Loopflow block above; keeps
  `docs: scripts/testing.md` and the one-build-at-a-time and headless rules.
- Sibling sentences brought in line: builtin `implement.md`, `compress.md`,
  `prompt.md` (gate selects a few high-risk tests including acceptance; CI
  runs the suites), `ci-fix.md` step 3 last bullet (a failure reaching CI is
  normal; add a local check only when it is cheap and would have caught it);
  `.lf/skills/implement.md`.
- `AGENTS.md`: the gate line in "When to run what", the gate comment in
  Development Environment, and "Scratch keeps one line per check" (the line
  now lives in the PR body once gate has cleared scratch).
- `.github/CI.md`: gate, not land, is where scratch is cleared and the
  candidate published; gate no longer "owns affected suites".
- `scripts/testing.md`: retitle "Gate: affected suites once per tree" to the
  focused local run and CI reproduction; the runner and `--reuse-passing`
  stay. "Plan first" suite notes remain as the map for choosing the few tests.
- `tests/goldens/builtin_implement.md` regenerated
  (`uv run python tests/goldens/update_goldens.py`).

### Docs written (write-docs, 2026-11-13)

The user docs and skills now describe the same sequence:

- `docs/ops.md` "Review while CI runs" (new): the gate sequence, a sample
  report by commit, fallbacks, and the agent-guide hook for repositories whose
  CI needs a preparation step.
- `docs/getting-started.md` §5, `docs/orchestration.md` (deliver), `README.md`
  and `docs/index.md` (`gate` comment, kept identical for
  `website/tests/test_readme_index_sync.py`).
- `.github/CI.md`, `scripts/testing.md`, `AGENTS.md`: done as listed above.
  `rust/loopflow/tests/README.md`, `website/README.md` and `tests/e2e/README.md`
  retain focused prompt/architecture preparation before gate; the e2e guide
  assigns full affected verification to CI.

The sample report in `docs/ops.md` is a sketch of the shape (tests with a
reason, one entry per head); the skill's wording decides the real one.

### Delete — do not maintain

Removed builtin and repo gate's local-suite-first instructions, changed-aware
runner default, rerun-by-input rules, "Phase 1 / Phase 2" structure and "Don't
start a second verification pass". No deletion targets remain. One build at a
time, headless checks, truthful deferrals, timeout investigation and deletion
of flaky tests survive on the CI-first path.

### Forbidden outcomes

- A local affected-suite pass kept "just in case" before or beside CI.
- `lf pr submit`, `lf pr arm` or `lf land` used to start CI.
- Blanket deletion of `scratch/`, or a scratch-clear instruction in the
  builtin.
- A report that calls skipped, pending or unavailable checks passing.
- A new notes file or directory standing in for scratch during gate.

### Exclusions

- LOO-883 (loop stopping) and PR #1915 (pursue order).
- `lf pr checks` reporting Passing for an all-skipped checkpoint; the skill
  reads the lines instead.
- `.lf/graphs/pursue-auto.yaml`: ends in `cmd: task pr land`, which is not a
  command, and runs `loop: implement` after gate. Stale; not repaired here.
- CI workflow, classifier and graph changes.

## Checks

- `cargo test -p loopflow --test golden_prompt` → pass (1 test), after snapshot regeneration; macOS linker warned about unwind-table size, with no check failure.
- `cargo test -p loopflow --test integration documented_commands::` → pass (4 tests).
- `uv run --project website --extra test pytest website/tests/test_readme_index_sync.py` → pass (1 test).
- `cargo package --locked --allow-dirty -p loopflow` → gate/CI owns the embedded-file packaging check; not run in implement.
- At gate, live: after the first publish,
  `test -z "$(find scratch -mindepth 1 ! -path 'scratch/.gitkeep' -print)"`
  holds, `gh pr view --json headRefOid -q .headRefOid` equals
  `git rev-parse HEAD`, and `gh pr checks` lists `rust-test` and
  `tests-result` as pending, not `skipping`; `gh pr view --json
  autoMergeRequest -q .autoMergeRequest` prints nothing.
- Before/after for the PR: minutes from gate start to the first suite job
  start on the PR (before: after `land`; after: target under 5 min).

- Compress: gate steps kept once in `.lf/skills/gate.md`, `.github/CI.md`
  links there; prose in unembedded files, no test rerun.

## Done when

- Gate on a configured repository has relevant CI running for its pushed head
  before it starts detailed review, and auto-merge is not requested.
- In Loopflow that run is the real matrix, and the plan is still readable from
  the checkpoint commit and the PR body.
- The builtin names no scratch cleanup; a repository with no PR or no CI gets
  a report that says no CI ran.
- Gate names a handful of tests with a reason each, and its report ties every
  result to a SHA, with pending, skipped and unavailable kept apart from
  passing.
- Implement, compress, AGENTS.md, CI.md and testing.md describe the same
  split: focused tests early, CI for suites, gate for review.

---
docs: scripts/testing.md
requires: code on branch
produces: published candidate, review fixes, and readiness report by head
action_style: procedural
---
Start CI, then make the branch ready to ship and easy to review while it runs.

CI is the test driver. Publish first so the suites run on real infrastructure,
and spend the wait reviewing instead of repeating them locally.

## Loopflow candidate preparation

Use the included `scripts/testing.md` to map changed paths to tests. CI skips
its suites while the branch has files in `scratch/` (`.github/CI.md`), so this
repository's preparation, at the first publish and after review fixes, is:

```bash
lf commit -m "gate: checkpoint"     # working notes are in history here
lf sync
lf scratch clear                    # branch-added files only; base files stay
lf pr publish                       # ready PR, no merge request
lf pr checks                        # rust-test, tests-result: not skipping
```

Before clearing, put what a later reader needs into the PR body: what people
said, what a run showed, open decisions, deferred checks and the checkpoint
SHA. The plan stays in this conversation and in
`git show <checkpoint-sha>:scratch/<file>`.

After clearing, write nothing under `scratch/`: the next publish would commit
it and CI would skip again. Findings and check results go in the report and PR
body.

If gate ends with work it cannot finish, restore the notes with
`git revert --no-edit <clear-commit>` and don't publish that commit. Local HEAD
is then a checkpoint for the next run, while the PR and its checks stay on the
candidate head: report the two separately.

Name modules in a local test filter so it finishes well inside nextest's
twenty-minute run limit (`.config/nextest.toml`). A test that fails in a loaded
run and passes alone failed from load: name it in the report and leave it to
CI.

1. **Publish a candidate.** Read the diff, uncommitted work included, and the
   plan in `scratch/`. Pick two to five tests for the riskiest behavior,
   including the plan's acceptance check, with a line on why each matters: a
   review focus, not a local matrix. Run the formatter, `lf commit`, `lf sync`,
   follow any preparation the agent guide names, then `lf pr publish`. Not
   `lf pr submit`, `lf arm` or `lf land`: gate asks for no merge.

2. **Confirm CI started for this head.** Compare the PR's `headRefOid` with
   local `HEAD` and read `lf pr checks` for that SHA. The suites that matter
   must be queued, running or finished; a passing classifier or a skipped suite
   ran nothing. If they never started, fix the cause or name them. With no
   remote or no CI, say so, run the selected tests locally and report that no
   CI ran.

3. **Review while CI runs.** Follow the change through its callers, data and
   failure paths, and fix what you find within the plan:
   - Security: authorization, secrets and data handling where touched.
   - Correctness and recovery, including partial completion.
   - Interfaces: one owner per responsibility, obvious signatures, the same
     word for the same concept.
   - Experience: fast paths stay fast; errors say what failed instead of
     silently losing work.
   - Leftovers: dead code, debug prints, resolved TODOs, unused shims, comments
     that restate code.
   - Docs: examples and commands match the final behavior.

   Run a test locally only for fast feedback on a fix, to reproduce a CI
   failure, or for a risk no suite covers; keep it small and inside the
   repository's time limits. One build at a time. A test that fails in CI and
   passes on a rerun is flaky: make it deterministic or delete it in this PR.
   Never rerun CI to get past a failure, and never report a deferred check as
   passed.

4. **Publish fixes once per pass.** Each push can cancel the previous run, so
   batch them: commit, sync, `lf pr publish`, confirm the new SHA and its
   checks. Don't push just to rerun CI. Keep the PR copy current.

Report the selected tests and why, the fixes, what is unresolved, the PR URL
and the published SHA. Tie every result to its head and tell passing, failing,
pending, skipped and unavailable apart; an earlier green head says nothing
about later edits. Don't wait for pending CI: hand off its state.
`lf land --wait-and-fix` owns the waiting.

Polish only what this branch changed. If it already meets the bar, leave it
alone. Commands belong in `scripts/testing.md`; module checks belong in the owning
module README.

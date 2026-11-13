---
requires: code on branch
produces: published candidate, review fixes, and readiness report by head
action_style: procedural
---
Start useful CI, then make the branch ready to ship and easy to review.

Read `scratch/` (`scratch/<branch>.md` is this PR's design, `scratch/questions.md`
holds open questions) and the repo's agent guide. Read Wave or PM context only
when the seed names the exact wave, Task or Project; never infer it.

## Workflow

1. **Prepare and start CI.** Read the diff summary, including uncommitted work,
   and the working notes far enough to publish one coherent candidate. Resolve
   the comparison from the working context; don't assume a base branch.
   - Identify two to five tests for the riskiest or most-edited behavior,
     including the design's automated acceptance check. Give one line of why
     each matters. Prefer existing CI coverage; this is a review focus, not
     another local matrix or a ledger of every test.
   - Use the formatter's auto-fix where needed, commit open work with
     `lf commit`, then `lf sync`. Follow any preparation the repository's agent
     guide names to make a PR one CI actually tests. Retain needed plans,
     findings, open decisions and check results before any cleanup.
   - Run `lf pr publish`, creating a ready PR if none exists. Start CI before
     detailed review or a local build/lint/test cycle. Don't use `lf pr submit`,
     `lf pr arm` or `lf land` to start CI; gate grants no merge approval.

2. **Confirm CI started for this head.** Compare the PR's `headRefOid` with
   local `HEAD`, then read `lf pr checks` and the workflow/check details for
   that SHA. Relevant suites must be queued, running or already completed.
   A passing classifier or suites marked skipped don't show that tests ran;
   don't trust the command's exit code alone. Fix the candidate/trigger cause
   when possible, otherwise name the suites that never started.
   - Without a remote, publishing access, `gh` or configured CI, say what is
     unavailable, run the selected tests locally and report "no CI ran" when
     that is known. If CI status can't be read, report it as unavailable.
     Keep local results separate from hosted results; don't claim publication
     when it failed.

3. **Review while CI runs.** Follow the change through its callers, data and
   failure paths. Fix concrete findings within the design:
   - Security: authorization, secret exposure and data handling where touched.
   - Correctness, failure handling and recovery, including partial completion.
   - Ownership and interfaces: clear responsibility, sensible defaults, obvious
     signatures, the same word for the same concept.
   - User experience: fast paths stay fast; errors explain what failed rather
     than silently losing work. Exercise changed interactions through headless
     UI tests or rendered snapshots; leave subjective walkthroughs to demo.
   - Maintainability: remove dead code, debug prints, resolved TODOs, unused
     compatibility shims and comments that only restate code. Add the why where
     it isn't obvious.
   - Docs: reread affected README/module docs and public API docs, including
     edits made during review. Examples and commands match the final behavior.

   CI is the primary test driver. Run a focused test locally only for fast
   feedback on a fix, to reproduce a CI failure, or to diagnose a risk no suite
   covers. Don't repeat unchanged passing work or run affected suites beside
   CI just in case. Add coverage for missing behavior; determine whether each
   failure is in the code or the test. Delete flaky tests rather than patching
   them. Investigate required-check timeouts instead of hiding them with a
   later green run.

   A local run is small and bounded. Select only what the fix reaches, keep
   inside the repository's test time limits, and never raise a limit or wait
   past one. A test that fails in a loaded run and passes when rerun alone
   failed from load: name it in the report and leave it to CI. Don't
   instrument it or rerun it to measure the race. A test that fails alone, or
   in CI, is the one to investigate.

   One build at a time: never run compilers, test runners or linters against
   the same build directory concurrently, including in background tool calls.
   Match CI's formatting/static analysis. Checks run headless; use a headless
   equivalent or defer unavailable checks to capable CI and say so. Never mark
   a deferred check passed.

4. **Publish fixes and report by head.** Batch review fixes into one publish
   per pass; each push can cancel the previous CI run. Commit, sync and repeat
   the repository's candidate preparation, then `lf pr publish`. Confirm the
   published SHA and relevant checks again. Don't push merely to rerun CI.
   - Keep PR context current: what improves, substantive findings, meaningful
     before/after measurements, check results and real limits. Delivery uses
     its pr-message template; reconcile existing or requested copy rather than
     writing a second report. Optional cached copy is `scratch/pr-title.txt`,
     `scratch/pr-body.md` and `scratch/.pr-copy-ref` (current HEAD SHA), consumed
     by publishing before its first commit or push.
   - Report the selected tests with their reasons, fixes and unresolved work,
     PR URL and published SHA. Tie every hosted and local result to its head;
     distinguish passing, failing, pending, skipped and unavailable. An earlier
     green head doesn't validate later edits or uncommitted changes.
   - Don't wait for pending CI to finish. Hand off its state honestly;
     `lf land --wait-and-fix` owns waiting and landing when the caller runs it.
     Gate itself requests neither merge nor auto-merge.

## Scope

Polish only code this branch changed, within the design. Skip unrelated
improvements and style preferences. If it already meets the bar, leave it
alone. Report check results and blockers briefly.

## Adaptation

Found a check this repo always needs (a formatter, a type check, a build step)?
Write it into the repo's agent guide or module docs so the next gate is faster.
Copy this skill to `.lf/skills/gate.md` when the repo needs gate to work
differently.

# LOO-617: Run CI's test jobs on published PRs

Status: draft plan (kickoff 2026-10-26). Direction accepted by Jack Heart
(2026-10-26): skipping CI test jobs on review PRs is not OK.

## Problem

`.github/workflows/ci.yml`'s `scratch-clear` job writes `candidate=false` for a
`pull_request` whose `scratch/` has files, and every proof job is gated on
`candidate == 'true'`; `tests-result` skips too. Review PRs keep scratch until
`lf land`/`lf pr submit` runs `lf scratch clear`, so hosted proof first runs at
land. PR #1677 (LOO-519) found a stale `ThirdPartyNotices/inventory.json` and a
broken `TerminalPaneLayoutTests` only at land; #1669 (LOO-576) skipped the same way.
The deferral came from #1361 (LOO-343, "Run hosted CI only on landing
candidates"); #1543 kept it.

## Demo

This PR is the demo: it is published with this design file in `scratch/` and
touches `.github/` (a macOS input), so its own checks show `rust-test`,
`swift-test`, `loopflow-ui-test` etc. running, and `scratch-clear` passing with
a notice naming the scratch files. `lf pr checks --required` shows `tests-result`
pass/fail from real results instead of "skipped".

## Approach

Delete the candidate concept. `scratch-clear` becomes a pure enforcer that
runs in parallel with the matrix:

- `pull_request` with scratch files: emit `::notice::` plus the file list to
  the step summary ("lf land clears these before the merge queue"), exit 0.
  No output.
- `merge_group`/`push` with scratch files, or missing `scratch/`: exit 1 (unchanged).
- Remove `outputs.candidate` and every `needs.scratch-clear.outputs.candidate`
  read. Proof jobs drop `scratch-clear` from `needs`; their `if` keeps
  `!cancelled()` (required because `merge-proof` is skipped off `push`) plus
  their remaining `merge-proof.reuse`/`macos-inputs`/`rust-build` conditions.
  Jobs whose only condition was candidate become `if: ${{ !cancelled() }}`.
- `tests-result`: `if: ${{ always() }}`; keeps `scratch-clear` in `needs` and
  in `check_result`, so a queue/main scratch violation still fails the aggregate.
  Replace the "skipped checkpoint" comment.

Unchanged cost controls: `macos-inputs` classification, `merge-proof` reuse on
main, `cancel-superseded`, PR `cancel-in-progress` concurrency.

## Risk findings

- **Auto-merge with scratch present.** A green PR `tests-result` no longer
  implies scratch is clear. Every arming path (`lf land`, `lf arm`,
  `lf pr submit`) runs `lf scratch clear` before pushing; a person enabling
  GitHub auto-merge by hand gets a queue entry that `scratch-clear` rejects.
  The queue stays the merge-time gate. Accepted.
- **Queue entry carrying scratch now burns runners** because the matrix no longer
  waits on `scratch-clear`. Only reachable by bypassing lf delivery; accepted
  in exchange for removing the serial classifier from every run's critical path.
- **Runner cost.** Every PR push now runs the matrix. Bounded by
  cancel-in-progress and macOS path classification; this is the cost Jack
  chose over late failures.
- `scratch/` paths are already in `_UNAFFECTED` in `scripts/macos_inputs.py`, so
  scratch-only pushes skip macOS jobs.

## Alternatives rejected

- Keep `scratch-clear` in `needs` gated on `result == 'success'` and make it pass
  on PRs: saves runners on bypassed queue entries but keeps a serial ~15 s job
  ahead of everything. Rejected for simplicity.
- Fail `scratch-clear` on PRs with scratch (pre-#1361 behavior): every review
  PR stays red for a reason that isn't a defect. Rejected.

## Scope

Affected surfaces:
- `.github/workflows/ci.yml` (above).
- `python/tests/test_ci_proof.py`: replace
  `test_checkpoint_defers_proof_but_queue_requires_clear_scratch` with a test
  that PR scratch exits 0 with a notice and no output, queue/main scratch and
  missing scratch exit 1; replace
  `test_matrix_only_runs_for_candidates_and_queue_cannot_skip_aggregate` with:
  no job reads `scratch-clear.outputs`, matrix jobs don't need `scratch-clear`,
  `tests-result` needs every job and its `if` is `${{ always() }}`; keep the
  macOS-classifier assertions.
- `TESTING.md` lines 3–21 (rewrite: PRs run the full matrix on each push;
  scratch is a merge-time requirement enforced by the queue), the
  `scratch-clear` table row, and "Candidate and merge-group jobs must all pass"
  (→ every job).
- `docs/architecture/delivery.md`: delete "Scratch cleanup selects landing
  candidates for this repository's hosted CI; PR readiness alone does not select CI."
- Builtin delivery skills: grep found no deferral wording; none change.

Exclusions: shipped release notes (`RELEASE_NOTES.md`, `release/*/NOTES.md`)
and Wave memory stay as written. No change to `lf scratch clear` or delivery code.

## Delete — do not maintain

- `scratch-clear.outputs.candidate`, `candidate=true/false` writes, the
  "hosted proof is deferred" summary line.
- The PR-checkpoint clause in `tests-result.if`.
- TESTING.md's checkpoint/candidate paragraphs and delivery.md's
  candidate-selection sentence.

Forbidden: a second switch (label, draft status, env flag) that re-skips tests on PRs.

## Done when

- `uv run pytest python/tests/test_ci_proof.py` passes.
- `actionlint .github/workflows/ci.yml` clean (if installed; otherwise CI).
- `git grep -n "candidate" .github/workflows/ci.yml` prints nothing.
- Hosted: this PR, with its scratch design present, runs `rust-test` and
  `swift-test` (not skipped); the merge queue would reject it until `lf land`
  clears scratch (covered by the merge_group cases in the Python test).

Check result: (not yet run)

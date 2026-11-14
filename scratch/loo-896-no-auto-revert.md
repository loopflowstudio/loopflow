# LOO-896: macOS on main reports red, never reverts

Direction (Jack Heart, comment): "just turn of the auto revert PRs"

## Why

One flaky `WorkCacheTests` failure on `286889bbec` (#1927, no Swift) bisected to
#1927 and opened revert #1929; the next two main pushes passed macOS with #1927
in. Bisection turns any flake into a blamed commit and a PR someone has to close.

## Outcome

`macos-main.yml` still runs after each push to main on the batch since
`last-green`. Green or skipped force-moves `last-green` to the tested commit.
Red (or cancelled) writes one job-summary line naming the commit and the
unchanged `last-green`, and the run fails. No dispatches, branches or PRs.

Demo: the settle step of the next red macOS-on-main run shows
`macOS failure on <sha>; last-green stays at <sha>.` and no new
`macos-revert/*` branch or PR appears.

## Approach

The remaining logic is two branches of shell, so it moves into the workflow
step and the script goes away.

- Delete `scripts/macos_main.py` and `python/tests/test_macos_main.py`
  (bisection, dispatch, revert, `_known_revert`, `failed_jobs`, `macos-revert/`
  prefix; nothing else imports them).
- `macos-main.yml`:
  - `on.workflow_dispatch` with no inputs (manual rerun of current main).
  - drop the bisect `run-name` and `inputs.request` from `concurrency.group`
    (`${{ github.workflow }}`).
  - `select` loses the dispatch branch: `SHA: ${{ github.sha }}`, always reads
    `last-green` and runs `macos_inputs.py`.
  - `settle`: runs for `push` and `workflow_dispatch`; drop
    `timeout-minutes: 340`, `actions: write`, `pull-requests: write`; keep
    `contents: write` for the tag push. Inline step:
    success|skipped → `git push --force origin "$SHA:refs/tags/last-green"`
    and summary `last-green → $SHA`; otherwise summary line above, `exit 1`.
    A full checkout is no longer needed beyond the push (keep `actions/checkout`
    default depth; pushing a SHA the remote has needs no history).
  - header comment: red is reported, not bisected.
- `.github/CI.md`: "Green moves `last-green`; red leaves it, fails the run and
  is reported in its summary. Nothing reverts automatically."

## Open revert branches on origin (2026-11-13)

| Branch | PR | State |
|---|---|---|
| macos-revert/286889bbec0c | #1929 | closed |
| macos-revert/aef0c0a5cd54 | #1808 | closed |
| macos-revert/dfd454cfc783 | #1819 | closed |
| macos-revert/62703f07e56b | #1931 | open |
| macos-revert/f7faf56f91b8 | #1883 | open |

Deleted the three closed branches 2026-11-13 (heads f2b744705d, 62d595f30c, 09a5fa0a2b; their PRs keep the commits). Leave #1931 and #1883 and their branches;
list them in the PR body (see `scratch/questions.md`).

## Exclusions

macOS job selection (`macos_inputs.py`, `macos.yml`), the merge queue, flake
repair of `WorkCacheTests`.

## Done when

- A red macOS-on-main run fails with the summary line and opens nothing.
- `grep -rn "macos_main\|macos-revert\|find_culprit\|macOS bisect" .` finds nothing outside git history.

## Docs

- `.github/CI.md` (macOS-on-main paragraph, written ahead of the workflow change). Summary line format there must match the settle step.

## Status

Implemented: workflow inlined, script and test deleted, CI.md updated.

## Checks

- `actionlint .github/workflows/macos-main.yml` — passed locally.
- `uv run pytest python/tests` collection still passes without the deleted test — gate.
- Real red/green settle — CI on main after merge.

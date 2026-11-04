# LOO-691: releases resume after their PR merges, and regressions live on GitHub

Jack Heart, 2026-11-04: fetch fix and GitHub regressions are one PR (#1780).
Notion removal and planning parity moved to LOO-726. Jack chose the installed
`lf` for host releases. Jack's dedicated machine already follows `dev`, so there is
no release lag, and
the separate release home is dropped.

## Done

Release fetches the revisions it builds or tags:
`release_run_fetches_a_release_merge_it_has_not_seen`.

## Experience

A regression is a GitHub issue on the repository being released:

```bash
gh issue create --title "Sessions lose their name" --label regressed-in:v0.15.3
```

While that issue is open, `stable` stays below v0.15.3. Closing it as completed
makes the first release published afterwards eligible once it soaks. Closing it
as not planned or as a duplicate, or removing the label, drops the hold. Every Machine reads the
same issues, so nothing depends on which Machine marked it or runs the release.
Jack Heart chose the issue as the record: Tasks carry no regression field.

## Build

- `regressions()` in `ops/release.rs` reads the repository's issues carrying a
  `regressed-in:vX.Y.Z` label through `gh`, in every state. Open holds; closed
  as completed gives `fixed_at` = closed time; closed as not planned is no
  regression. A label that does not parse as a version is ignored, as today.
- Unreadable GitHub: the release publishes and `stable` holds, with the reason
  in the run log and `lf release status` (`RegressionsUnread`). `lf release
  check` fails when this home cannot read them, for `completion:
  github-release` targets. `loo-691-combined` has this shape; reuse it.
- Remove regressions from Task management: `--regressed-in` on `lf task
  create|edit`, `regressed_in` on Task, `TaskUpdate`, DTOs and fixtures, Swift
  (`WaveWorkMap.swift`), `Edit`/`WorkValue::TaskRegressedIn`,
  `regression_tasks`, `planning_explain`, and their tests.
- `tasks.regressed_in` shipped in v0.15.1. Nothing reads it now; do not drop it
  here (destructive schema changes are staged). Released `lf` requires
  `task_regressed_in` on every Task it imports from a Git plan ref, so it
  round-trips unread as `HistoricalTaskRegressedIn`, as `task_paused` does.
  LOO-727 removes it after those builds exit.
- Docs: `release/README.md` (mark, clear, soak) and the `release-run` skill.
  Delete "It reads the release Machine's Tasks, never Linear."

## Proof

release_tests with the fake `gh`: an open labelled issue holds `stable`; closed
as completed releases the next soaked release; closed as not planned does not
hold; a failing `gh issue` read publishes, holds `stable` and fails `release
check`. A released mark crosses the plan ref unread
(`a_released_regression_mark_round_trips_the_plan_ref_unread`).

Check: `release_tests` 80/80, focused lib and `dto_fixtures` passing; affected
suites and Swift belong to gate.

## Not in this diff

- Notion and planning parity (LOO-726).
- Dropping `tasks.regressed_in` after old builds exit (LOO-727).

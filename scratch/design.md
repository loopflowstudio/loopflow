# LOO-691: releases resume after their PR merges, and regressions live on GitHub

Jack Heart, 2026-11-04: fetch fix and GitHub regressions are one PR (#1780).
Notion removal and planning parity moved to LOO-726. The separate release home
is out of this diff; `loo-691-combined` (e8ffd80ca0, local) keeps that work.

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
as not planned, or removing the label, drops the hold. Every Machine reads the
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
- `tasks.regressed_in` shipped in v0.15.1. Stop reading and writing it; do not
  drop it here (destructive schema changes are staged). A released `lf` may
  still put `TaskRegressedIn` on a Git plan ref, so this build must read such a
  document and ignore the value. Check how unknown WorkValues decode.
- Docs: `release/README.md` (mark, clear, soak) and the `release-run` skill.
  Delete "It reads the release Machine's Tasks, never Linear."

## Proof

release_tests with the fake `gh`: an open labelled issue holds `stable`; closed
as completed releases the next soaked release; closed as not planned does not
hold; a failing `gh issue` read publishes, holds `stable` and fails `release
check`. A store fixture at v0.15.1 with `regressed_in` set still opens.

## Not in this diff

- The release home (undecided), Notion, planning parity (LOO-726).
- Dropping `tasks.regressed_in` after old builds exit.

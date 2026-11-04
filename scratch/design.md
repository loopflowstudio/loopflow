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
gh label create regressed-in:v0.15.3
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
  check` fails when this Machine cannot read them, for `completion:
  github-release` targets. `loo-691-combined` has this shape; reuse it.
- Remove regressions from Task management: `--regressed-in` on `lf task
  create|edit`, `regressed_in` on Task, `TaskUpdate`, DTOs and fixtures, Swift
  (`WaveWorkMap.swift`), `Edit`/`WorkValue::TaskRegressedIn`,
  `regression_tasks`, `planning_explain`, and their tests.
- `tasks.regressed_in` shipped in v0.15.1. Nothing reads it now; do not drop it
  here (destructive schema changes are staged). LOO-730 drops it 30 days after
  the release carrying this.
- Docs: `release/README.md` (mark, clear, soak) and the `release-run` skill.
  Delete "It reads the release Machine's Tasks, never Linear."

## Proof

release_tests with the fake `gh`: an open labelled issue holds `stable`; closed
as completed releases the next soaked release; closed as not planned does not
hold; a failing `gh issue` read publishes, holds `stable` and fails `release
check`.

Check: see the last line of this file; affected suites and Swift belong to gate.

## Not in this diff

- Notion and planning parity (LOO-726).
- Dropping `tasks.regressed_in` (LOO-730).

## Next (Jack Heart, 2026-11-04)

Decisions, in this PR:

1. **No regression code left in Tasks or the plan exchange.** Delete
   `WorkValue::HistoricalTaskRegressedIn`, its `planning_write.rs` arms, the
   `optional_task_column` read and
   `a_released_regression_mark_round_trips_the_plan_ref_unread`. No schema
   migration: `tasks.regressed_in` stays in migrated databases, unused and
   unnamed by code. LOO-730 drops it 30 days after the release carrying this. Checked
   2026-11-04: every repository with a plan ref is configured `provider: linear`.
2. **`lf release` owns regressions and promotion.** `stable` stays GitHub's
   Latest marker.

```bash
lf release issues v0.15.3          # what is reported against this release, and whether it holds stable
lf release issues                  # every release with open regressions
lf release regress v0.15.3 "Sessions lose their name"   # creates the label if needed, files the issue
lf release promote                 # checks soak and regressions, then moves stable
lf release promote --dry-run       # what would move, or what holds it
```

- `issues` reads through the existing `regressions()` reader, keeping each
  issue's number, title, state and URL so it can list them; `--json` like the
  other release reads where they have it.
- `regress` validates the version, creates the label when missing, files the
  issue through `gh`, and prints its URL. A marked existing issue stays
  `gh issue edit --add-label`.
- `promote` is `advance_stable` as a command, including the publisher's
  `promote --tag`. `lf release run` calls the same path after each run: one
  implementation. Unreadable regressions hold and fail the command with the
  reason. It never moves `stable` back. `--dry-run` writes nothing.
- Docs: lead `release/README.md` and the `release-run` skill with these
  commands instead of raw `gh`, keeping a one-line note that a labelled issue is
  all it takes.

Proof: release_tests with the fake `gh` for each command: `issues` filters by
version and shows the hold; `regress` creates a missing label once and files
the issue; `promote` moves past soak, stops at an open regression, and
`--dry-run` leaves Latest unchanged; `release run` still promotes.

Check: `cargo test -p loopflow --test release_tests -- stable issues regress promote` 15/15; `--lib ops::release::` 48/48; clippy and affected suites deferred to lint/gate.

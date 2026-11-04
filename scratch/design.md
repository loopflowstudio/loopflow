# LOO-691: releases resume after their PR merges, and regressions live on GitHub

Jack Heart, 2026-11-04: fetch fix, GitHub regressions and the `lf release`
regression commands are one PR (#1780). Notion removal and planning parity moved
to LOO-726. Jack chose the installed `lf` for host releases. Jack's dedicated
machine already follows `dev`, so there is no release lag, and the separate
release home is dropped.

## Experience

```bash
lf release regress v0.15.3 "Sessions lose their name"   # creates the label if needed, files the issue
lf release issues v0.15.3                               # what it carries, and what each issue holds
lf release issues                                       # every open regression
lf release promote --dry-run                            # what would move, or what holds it
lf release promote                                      # checks soak and regressions, then moves stable
```

A regression is a GitHub issue on the repository being released, labelled
`regressed-in:vX.Y.Z`; the label is all it takes, so `gh issue edit --add-label`
marks an existing bug. While that issue is open, `stable` stays below v0.15.3.
Closing it as completed makes the first release published afterwards eligible
once it soaks. Closing it as not planned or as a duplicate, or removing the
label, drops the hold. Every Machine reads the same issues, so nothing depends on
which Machine marked it or runs the release. `stable` stays GitHub's Latest
marker and never moves back: a regression filed against the release `stable`
already points at keeps newer releases held and leaves `stable` there. Jack Heart
chose the issue as the record: Tasks carry no regression field.

## Done

All in the branch, checked against the code 2026-11-04:

- Release fetches the revisions it builds or tags:
  `release_run_fetches_a_release_merge_it_has_not_seen`.
- `regressions()` in `ops/release.rs` reads labelled issues through `gh`, in
  every state. Unreadable GitHub: the release publishes, `stable` holds with the
  reason in the run log and `lf release status` (`RegressionsUnread`), and
  `lf release check` and `lf release promote` fail.
- `lf release issues`, `regress` and `promote`. `lf release run` and `promote`
  share `advance_stable`, including the publisher's `promote --tag`.
- Regressions left Task management: `--regressed-in`, `regressed_in` on Task,
  DTOs, fixtures and Swift. No schema migration.
- `WorkValue::HistoricalTaskRegressedIn` carries `task_regressed_in` through the
  plan exchange unread, the way `HistoricalTaskPaused` does, so stored
  `planning_peer_changes` rows and released peers keep decoding. Jack Heart chose
  this on 2026-11-04 over removing it: every `lf` since v0.15.1 stores the field,
  and Git planning failed on it. Proved by
  `a_stored_released_regression_row_leaves_peer_planning_readable` and
  `a_released_regression_mark_round_trips_the_plan_ref_unread`.
- `release/README.md` and the `release-run` skill lead with the commands.
  `release/README.md` says the dedicated host follows `dev`.

## Remaining

- Gate: affected suites and Swift.
- The PR body predates the commands and names the abandoned column-drop Task.
  pr-publish rewrites it with LOO-730, which deletes the variant, the
  `task_regressed_in` rows and the column together, since scratch clears.
- Unproved: a real release resuming on the dedicated host after its PR merges,
  without a manual fetch. Needs this fix to reach that host through `dev`.
- Unproved: the commands against real GitHub; only the fake `gh` has run them.
  Checked 2026-11-04 with read-only `gh`: Issues are enabled on the repository, and
  `state`/`stateReason` arrive as the `CLOSED`, `COMPLETED`, `NOT_PLANNED` and
  `DUPLICATE` the code matches. Label creation and filing remain unproved.

## Not in this diff

- Notion and planning parity (LOO-726).
- Dropping `tasks.regressed_in` (LOO-730), 30 days after the release carrying this.
  LOO-730 also deletes `HistoricalTaskRegressedIn` and the `task_regressed_in` rows
  in `planning_peer_changes`.

## Proposed Wave memory

Standalone realign leaves `.lf/wave/` read-only. For `release/MEMORY.md`: `stable`
holds come from `regressed-in:vX.Y.Z` GitHub issues, not any Machine's store;
release state read from one Machine's Tasks fails silently on every other.

Check: `cargo test -p loopflow --lib ops::release::` 48/48; `--test release_tests` 88/88; `planning_write::` 6/6 reused; clippy and affected suites deferred to lint/gate.

# LOO-691: release home on dev, regressions from Linear

Accepted by Jack Heart, 2026-10-30, in the LOO-691 concept review
(`scratch/concept-review.html`, `scratch/questions.md`).

## Experience

The release host releases with main's release code, without touching Jack's
main home. A bad main build is fixed by reinstalling the release home only.

`stable` never moves past an open regression. A regression is marked once
(`lf task create|edit --regressed-in v0.15.3`) and every home sees it, because
the release reads regressions from Linear, not from its own store.

## Done

- Release fetches the revisions it builds or tags (#1780 contents), with
  `release_run_fetches_a_release_merge_it_has_not_seen`.
- Regression = `Regressed in › vX.Y.Z` label on the issue (label group, one
  version per issue). `--regressed-in` writes it through `LinearClient::
  mark_regressed_in` before the local save; a Task with no Linear issue, or a
  repository without `planning.team`, is refused.
- `regressions()` in `ops/release.rs` reads the Team (from `.lf/config.yaml`
  `planning.team`) for labelled, non-canceled issues; `fixed_at` is
  `completedAt`. Unread Linear: the release publishes, `stable` holds with a
  `RegressionsUnread` reason in the run log and `lf release status`.
- `release/README.md` and the `release-run` skill describe Linear regressions;
  local-only regression tests are removed.
- `lf release check` reads Linear regressions for a `completion: github-release`
  target and fails, naming the reason, when this home cannot
  (`release_check_reports_a_home_that_cannot_read_linear_regressions`).
- `lf task edit --regressed-in` through the ops path labels the issue, replaces an
  earlier version, and refuses a Task with no Linear issue or a repository with
  no `planning.team` (`task_edit_regressed_in_*` on the stateful Linear fake).
- Host, option (b), recorded as an assumption in `questions.md`.
  `scripts/bootstrap-cron-host.sh` builds `lf` from origin/main in a clone at
  `$LF_RELEASE_HOME/src` (default `~/.lf-release`) with `scripts/install.py
  local`. release-run's wrapper runs that `lf` with `LF_HOME`/`LF_BIN` set to
  the release home. Bootstrap verifies the release home's provider login and its
  `lf release check`. telemetry-daily stays on the installed `lf`.

## Remaining

- Observation projecting the label into `tasks.regressed_in`: LOO-695
  (Infrastructure). Release behavior does not depend on it.
- Demo, on the release host: first bootstrap, release-home logins
  (`account connect linear`, provider), and one scheduled release-run under the
  release home. The live-Linear regression query also runs here. Not runnable
  headless.
- Gate/CI.

## Preserve

- Existing release-run behavior: candidates, resume, repair, notes.
- Jack's main home is never opened by the release job.
- No regressions were marked as of 2026-10-30, so no data migration.

## After landing (Jack chose 2b)

Rerun `scripts/bootstrap-cron-host.sh infrastructure` on the release host. It
builds this change into the release home, and the next daily release uses the
fetch fix.

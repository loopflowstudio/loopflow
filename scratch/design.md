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

## Remaining

- **Host (blocked on Jack Heart's choice, see `questions.md` counterexample).**
  Installs are per OS account, not per `LF_HOME`, so a release home cannot
  hold its own `dev` build. Until chosen, `scripts/bootstrap-cron-host.sh` is
  unchanged and `release/README.md` says the host runs its installed `lf`.
- Observation does not bring the label back into `tasks.regressed_in`
  (needs a `pm_items` column + migration); other homes' local display lacks
  the mark. Release behavior is unaffected. Do here or file a Task.
- `lf release check` does not verify Linear auth (design asked for it).
- Proof gaps: no `lf task edit --regressed-in` test through the ops path (the
  label write is tested at `LinearClient`); the regression query is unexercised
  against live Linear.
- Gate/CI.

## Preserve

- Existing release-run behavior: candidates, resume, repair, notes.
- Jack's main home is never opened by the release job.
- No regressions were marked as of 2026-10-30, so no data migration.

## After landing (Jack chose 2b)

Install the `dev` build that contains this change into whichever home the host
releases from, so the next daily release uses the fetch fix.

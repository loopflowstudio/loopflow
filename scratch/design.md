# LOO-691: release home on dev, regressions from Linear

Accepted by Jack Heart, 2026-10-30, in the LOO-691 concept review
(`scratch/concept-review.html`, `scratch/questions.md`). Done already: release
fetches revisions it is about to build or tag (PR #1780 contents).

Status (2026-10-30): Model implemented, with the deviations recorded in
`scratch/questions.md`. Host is blocked by a counterexample: installations are
per OS account, not per `LF_HOME`, so a release home cannot follow `dev` on its
own; it needs Jack Heart's choice there.

## Experience

The release host releases with main's release code, without touching Jack's
main home. The `release-run` job runs `lf` with its own `LF_HOME` (proposed:
`~/.lf-release`) installed from `dev`. A bad main build is fixed by
reinstalling that home only. Release-notes Sessions and release-PR repairs
live in the release home.

`stable` still never moves past an open regression. A regression is marked once
(`lf task create|edit --regressed-in v0.15.3`) and every home sees it, because
the release reads regressions from Linear, not from its own store.

## Model

- Regression in Linear: proposed as a label group `Regressed in` with one child
  label per version (`v0.15.3`) on the issue. Linear has no custom fields; a
  group keeps one version per issue. Implementation may choose another
  representation only if it is as readable in Linear and one version per issue.
- `--regressed-in` writes that label through the Task's existing Linear write
  path; observation brings it back into `tasks.regressed_in`, which keeps
  working for local display. No local-only regressions remain.
- `regressions()` in `ops/release.rs` queries Linear for the repository's
  issues carrying a `Regressed in` label, not canceled; `fixed_at` is
  `completedAt`. The release home needs Linear auth; `lf release check`
  verifies it with the other host credentials.
- Linear unreachable or unauthenticated: the release still publishes, `stable`
  does not move, and the run logs why (fail closed: a stalled `stable` is the
  failure, not a bug).
- The release home has no Task plan, so how it maps the repository to its
  Linear team/issues needs a source it can read without a synced store
  (repository config or the existing Wave `linear:` ids). Decide in implement;
  record the choice in `scratch/questions.md`.

## Host

- `scripts/bootstrap-cron-host.sh`: the `release-run` job runs with the release
  `LF_HOME` and its `lf`; bootstrap installs `dev` into it and runs its
  `release check`. `telemetry-daily` stays on the main home.
- `release/README.md`: replace "It reads the release Machine's Tasks, never
  Linear" and the paragraph on which `lf` the host runs. `release/SCHEDULE.md`
  if it names the job's home.

## Preserve

- Existing release-run behavior: candidates, resume, repair, notes.
- Jack's main home: its store, stream and Sessions are never opened by the
  release job.
- No regressions are marked today (checked 2026-10-30), so no data migration.

## Proof

- release_tests: a Linear double with an open regression holds `stable`; a
  completed one releases the next soaked release; Linear failure holds
  `stable` and still publishes.
- Task edit test: `--regressed-in` produces the Linear label write.
- Bootstrap script check (existing script tests, if any) for the release home.

## After landing (Jack chose 2b)

Install the `dev` build that contains this change into the host's release
home, so the next daily release uses the fetch fix.

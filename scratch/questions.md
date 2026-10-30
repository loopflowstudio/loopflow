# LOO-691 questions

## Status (2026-10-30)

Plan of record: `scratch/design.md` (Done / Remaining).

Check: `cargo test -p loopflow --lib -- linear::regressions:: ops::release::tests:: task_edit_regressed_in task_create_in_linear_repository` — 56 pass after the compress pass; `cargo clippy --all-targets -- -D warnings` clean; `release_tests -- daily_run_moves_stable release_check` — 6 pass (prior pass; integration paths unchanged).

## Host: option (b) taken (reversible; Jack Heart can overturn in review)

Installs are per OS account (`~/.lf-machine/install`), so a release `LF_HOME`
cannot hold an installed `dev` build. The release home instead runs an `lf`
built from origin/main in a clone at `~/.lf-release/src`
(`scripts/install.py local`), with `LF_HOME`/`LF_BIN` pointing at it. Checked as
safe for the main home: a custom home's paths that read the account store
(`build_cleanup_registries`, worktree prune guards) open it read-only without
migrating, and child `lf` resolves through `LF_BIN`. A clone, not a worktree, keeps the main
checkout's worktree pruning away from it.
- The release home follows main only when bootstrap reruns. Rebuilding before
  each scheduled run is the alternative; it was not chosen because a failed
  build would block that day's release.
- The release home needs its own provider and Linear logins. Copying a store
  would share rotating OAuth grants between homes.
- Unverified headless: a fresh release home resolving Wave `infrastructure` for
  `lf --wave infrastructure --batch graph release-run` (demo).

## Assumptions (reversible)

- `--regressed-in` labels the issue directly through `LinearClient`
  (`mark_regressed_in`) after validating, before the local save; not through
  planning-change receipts. Projecting the label back into `tasks.regressed_in`
  is LOO-695.
- A repository without Linear planning (`planning.team`) has no regressions;
  `stable` moves on soak alone there. `--regressed-in` there is refused.
- Repository → Linear mapping comes from `.lf/config.yaml` `planning.team`,
  readable without a synced store. Linear auth comes from the home's store.
- `lf release check` verifies only that regressions are readable (auth and query)
  for `completion: github-release` targets; other targets skip Linear.
- Regression query filters `labels: {some: {parent: {name: {eq: "Regressed in"}}}}`
  and includes archived issues; not yet exercised against live Linear.

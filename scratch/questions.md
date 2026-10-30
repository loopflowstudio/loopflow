# LOO-691 questions

## Status (2026-10-30)

Done: release fetches the revisions it builds or tags (#1780 contents).
Done: regressions read from Linear (`Regressed in` label group, one version
label per issue); `--regressed-in` writes that label; unread Linear holds
`stable` with a logged reason and the release still publishes.
Remains: the host half of the design (see counterexample); gate/CI.

Check: `cargo test -p loopflow --lib -- linear::regressions:: ops::release::tests::` — pass; `daily_run_moves_stable` (release_tests) — pass; clippy clean.
Compress check: `cargo test -p loopflow --lib -- ops::task::` — pass (34); clippy clean.

## Counterexample: release home cannot follow `dev` on its own (blocking host half)

`installation::root_for_home` places the install under the OS account
(`~/.lf-machine/install`, "outside every loopflow home"). A release `LF_HOME`
cannot hold its own `dev` install: `lf install --dev` run for it switches the
account's one `lf`, so Jack Heart's main home would be opened by a `dev` build
and migrated forward. `scripts/bootstrap-cron-host.sh` is unchanged. Choices
for Jack Heart:
- a) release home with its own `LF_HOME` on the account's installed `lf`
  (store/Session isolation, no `dev` code; needs its own provider and Linear
  auth).
- b) release job runs a separately built `lf` (e.g. `scripts/install.py`
  `local-bin/` from origin/main) under the release `LF_HOME`.
- c) per-home installations (install model change; its own Task).
2b (install a `dev` build after #1780 lands) still applies to whichever home
the host releases from.

## Assumptions (reversible)

- `--regressed-in` labels the issue directly through `LinearClient`
  (`mark_regressed_in`) after validating, before the local save; not through
  planning-change receipts. Linear observation does not yet bring labels back
  into `tasks.regressed_in` (needs a `pm_items` column and migration), so
  another home's local display lacks the mark. The release reads Linear, so
  `stable` is unaffected.
- A repository without Linear planning (`planning.team`) has no regressions;
  `stable` moves on soak alone there. `--regressed-in` there is refused.
- Repository → Linear mapping comes from `.lf/config.yaml` `planning.team`,
  readable without a synced store. Linear auth comes from the home's store.
- `lf release check` (the change-set command) does not verify Linear auth; an
  unauthenticated release home shows up as a `stable` hold in the run log and
  `lf release status`.
- Regression query filters `labels: {some: {parent: {name: {eq: "Regressed in"}}}}`
  and includes archived issues; not yet exercised against live Linear.

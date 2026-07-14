# Open questions — W2-133

## A source build cannot open the live `~/.lf/loopflow.db` (pre-existing)

The installed `lf` 0.10.1 records the baseline migration as `0.10.001_initial`;
`store/migrations.rs` on `main` (a438d2c9) expects exactly `001_initial`, so a
binary built from this branch calls the real registry "incompatible" and reports
`No wave registry on this machine yet.` Reproduces on `lf ls` with an unmodified
tree, so it predates this Task and is out of its scope (the directive says not to
repair unrelated infrastructure).

Worked around for dogfooding: copy `~/.lf/loopflow.db` to a temp `LF_HOME` and
rewrite the version row. The real registry was not touched. Someone owns
reconciling the installed binary with `main`'s baseline version string —
otherwise no from-source build can read the machine's own waves.

## Assumption: attention comes only from Sessions

An unstarted Linear task is not "waiting on you", so plan rows with no Session
never enter `attention`. If a wave wants "this backlog item has sat for a month"
that is a different, PM-grade check (`pm doctor`-class), not machine state.

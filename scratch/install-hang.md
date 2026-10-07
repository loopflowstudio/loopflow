# Installation pauses on a busy, large Home

Jack Heart reported `lf install` appearing hung on October 6, 2026.

The installed v0.13.5 command reproduced the delay installing published v0.13.6.
The owned installation completed successfully after several minutes, including
the supported store migration and Desktop replacement. CLI and app both report
0.13.6; a second installed `lf install` returned “already installed.” No process
was killed and no database or installation receipt was edited directly.

Samples `/tmp/infra-install-hang-sample.txt` and
`/tmp/infra-install-helper-sample.txt` show repeated bundled-helper preflights
copying the live database. Incremental backup had no pinned read transaction.
An isolated SQLite reproduction with a write between chunks failed to finish
after 200 steps; a transaction plus an actual schema read completed in 19.
BEGIN alone did not fix it. The Rust regression uses a 64 MiB WAL database with
continuous writes and checks completion before writes stop, retained contents,
and integrity. It passed in 0.81 seconds.

`/tmp/infra-install-switch-sample.txt` separately shows migration backup sleeping
between 64-page chunks. The installed store was 2.2 GiB. The migration already
holds its writer exclusion, so these sleeps only prolong installation and that
exclusion. Increase batches to 4096 pages with no deliberate delay and print
the backup phase. Keep the pre-migration backup and atomic publication intact.

Local repair pins the preflight snapshot before copying. The successful installed
replay used v0.13.6, not this unshipped repair. Remaining: delivery of the prevention fix. Repeated full preflight
copies remain a possible further optimization, outside this small repair.

Checks: candidate snapshot regression, migration backup/exclusive transaction,
complete previous-generation backup, `cargo fmt --all`, and
`cargo clippy --all-targets -- -D warnings` passed. Review retained the existing
backup publication and migration authority; no schema or live-state bypass.
`cargo test -p loopflow --test local_promotion` also passed 2/2, including
selected-store nonmutation. Publication/landing approval was requested in this
conversation after these checks; no external delivery has occurred.

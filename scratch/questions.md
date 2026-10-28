# LOO-647 decisions and open questions

- `lf disk` footprint report deleted along with `lf disk clean`, `disk.rs` and the
  in-process rotation thread: the script reports bytes freed per step and per
  repository, and nothing else read the footprint. Reversible.
- Store bookkeeping (dead Process rows and receipts, usage compaction, vacuum)
  needs SQLite and lf's schema, so it stays in the binary as `lf monitor prune`,
  which the script runs first. It no longer runs hourly inside capturing
  processes; it runs whenever the cron job does.
- Home retention is 7 days for every rotatable category (what hourly
  `lf disk clean --keep-days 7` already enforced), not the thread's 30.
- The five-second inspection failure was `build_cleanup`'s lsof/tree-walk
  deadline; raised to 180 s. `ROOTS` gained `node_modules` and `.venv`.
- Cron cadence: hourly at :17, one agent run per hour. Cheaper would be the
  script hourly and the skill daily; kept Jack's single-entry shape. With deletion
  deterministic the agent is only a reporter, so two cron lines would cost one
  agent run a day instead of 24. Recorded for Jack; not blocking.
- Unattended safety: the script does all deletion, including ignored
  build output untouched for four hours in idle linked and main checkouts; the
  skill only reports. Uncertain cases are kept and reported.
- Snapshot staging directories (`~/.lf/copies/.tmp*`) left by a killed draft
  build now go when the next snapshot starts or, unheld, after a week; the
  removed copies-lock check let `lf disk clean` take them at any age.
- Shipped release notes (`RELEASE_NOTES.md`, `release/*/NOTES.md`) still name
  `lf disk`; they are shipped records and stay as written.
- Codex account homes (`~/.lf/accounts/codex/*`) are not rotated: account
  homes are on the never-remove list, so not trivially covered.
- Not yet done: the acceptance run on the laptop
  (`cd ~/src/loopflow && lf skill disk-cleanup`) and `scripts/disk-cleanup.sh install`
  need this branch's `lf monitor prune` (with store retention) installed; run them after landing.
- Realign 2026-10-27 (three passes): plan and diff agree. The second concept review's five items (one pruner in `lf monitor prune`, deleting `lf self prune`, the script step, regenerated docs, Disk cleanup row and `DerivedData`) are all in the checkout as of 12:38. Main has not moved past the base.
- Compress check: `cargo clippy -p loopflow --all-targets -D warnings` clean; `cargo test --lib store::copy` 3 passed; CLI reference regenerated. Gate owns the Python script test and full suites.

## Concept review 2026-10-27: done

- The idle-checkout rule moved from the skill into `idle_builds` (main and
  linked worktrees under `~/src`); skill section 2 is report-only.
- Check: `bash -n`, `shellcheck` clean; `pytest python/tests/test_disk_cleanup.py` 3 passed.

## Concept review 2026-10-27 (second pass): done

- One Process-record pruner: `lf monitor prune` now runs store retention
  (close proven-dead rows, forget finished unreferenced rows, usage compaction,
  vacuum) after reaping and before receipt removal; `ProcessPruneReport.store`
  is `null` on a dry run. `lf self prune`, `commands/prune.rs` and
  `prune_dead_process_receipts` are deleted; the script runs `lf monitor prune`.
- Assumption, reversible: hourly cron also reaps registered orphan provider
  groups whose owning lf process is proven dead (the command's existing
  promise). Recorded for Jack.
- The monitor prune integration test now expects a Process-named receipt that
  proves death to settle and forget its unfinished row; a PID-named receipt
  never linked to the row keeps it unknown.
- LOO-643 (gate, unpublished) adds `repair_unrecorded_ancestry` in `run_prune`
  before the report is built. That hunk is separate from this one, and its
  repair runs before retention forgets rows. Whichever lands second keeps both
  outcomes in the one path and keeps dry runs non-mutating.
- Check: clippy -D warnings clean; `cargo test --lib store::sqlite::retention lf::commands::top` 7 passed;
  `--test process_ownership_tests monitor_prune` 1 passed; pytest test_disk_cleanup 3 passed; shellcheck clean; CLI reference regenerated.

## Compress 2026-10-27

- `RetentionReport.processes` → `forgotten_processes`: it is now on the `lf monitor prune --json` wire beside `closed_processes`.
- Check: clippy -D warnings clean; `--lib store::sqlite::retention lf::commands::top` 7 passed; `process_ownership_tests monitor_prune` 1 passed; pytest test_disk_cleanup 3 passed; grep for `self prune|prune_dead_process_receipts` empty.

## Concept review 2026-10-27 (third pass): nothing to change

- Concepts settled: the script deletes, the skill reports, cron runs the skill through `disk-cleanup.sh skill`, and `lf monitor prune` is the one lf entry point. No `lf disk`, `disk::` or launchd reference remains outside the uninstall line and shipped release notes.
- Open but not blocking (Jack's chosen shape): an hourly agent run that only reports; script hourly plus skill daily would be cheaper.

## Gate 2026-10-27

- Comparison: `origin/main...HEAD` after the recorded main merge; 21 Task paths.
  The four-hour build policy is retained from commit `1c7442ea33`, whose message
  records Jack's sweep and decision; it supersedes the earlier day-based notes.
- Review repairs: keep backups with open or recent sidecars; keep held caches
  and Trash and inspect their full age; retain on failed modification or Git
  inspection; refresh the process view between steps and before each checkout;
  retain all output of a checkout held by a process. Log mechanical steps and
  failures, preserve launchd if cron installation fails, and never steal a
  four-hour-old lock from a possibly live cleanup.
- Verification plan: changed-aware runner with reuse, architecture, cleanup
  Python tests, website docs tests, and Rust filters for build cleanup, store
  copy/retention, monitor, CLI discovery/reference and documented commands.
  Rust formatting and workspace Clippy follow tests. The full Rust matrix
  remains CI-owned; laptop cleanup and cron installation remain post-landing
  acceptance, as recorded above.
- `uv run python scripts/test.py --reuse-passing --rust-filter 'test(build_cleanup) | test(store::copy::) | test(store::sqlite::retention::) | test(lf::commands::top::) | test(monitor_prune) | test(cli_discovery::) | test(cli_reference::) | test(documented_commands::)'` — PASS: architecture, 8 cleanup tests, 63 Rust tests, formatting, workspace Clippy `-D warnings`, 72 website tests; 300s, no timeouts. Full Rust matrix remains CI-owned.
- `uv run --no-sync python scripts/test_network.py uv run --no-sync pytest python/tests/test_disk_cleanup.py` — PASS: 9 tests in 42.30s after final script simplification and the checkout-cwd regression; unchanged Rust and website inputs retain the preceding results.
- `bash -n scripts/disk-cleanup.sh`, `shellcheck scripts/disk-cleanup.sh`, Ruff check/format for `python/tests/test_disk_cleanup.py`, and `git diff --check` — PASS.
- No gate blocker remains. Configured laptop cleanup, per-repository reclaimed-byte evidence and cron installation are still required at the existing post-landing verification boundary; no real cleanup or schedule mutation was performed by gate.

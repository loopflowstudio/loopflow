# LOO-693 demo: skew under a newer store (2026-11-02, headless)

Branch build `lf 0.15.1+0461098ce` (`target/debug/lf`), run through the real
commands against a private `LF_HOME` whose store carries a future migration
row (`9.0.001_future`), the same fixture `doctor_tests::status_reads_name_a_newer_store_instead_of_reading_empty` uses.
No one was available; this pass stood in for Jack Heart. Page: `scratch/demo.html`.
Design: `scratch/questions.md` (plan and the 2026-11-02 concept review).

## Observed

| Command (branch lf, skewed store) | Exit | Output |
| --- | --- | --- |
| `lf wave status` | 1 | stderr: `database migration 9.0.001_future is unknown to lf 0.15.1+0461098ce (latest known task_regressed_in); a newer lf migrated this store; run `lf install`` |
| `lf wave status --json` | 1 | same refusal, nothing on stdout |
| `lf wave list` | 1 | same refusal |
| `lf task status` | 1 | same refusal |
| `lf self doctor` | 1 | still reports: `migrations: applied 9.0.001_future / known task_regressed_in`, `this store needs an lf that knows 9.0.001_future … run `lf install`` |
| `lf pr publish` (throwaway repo) | 1 | `Task PR authority refused: the shared Loopflow registry <path> cannot be opened: <store error ending in run `lf install`>` |
| `lf pr submit` (throwaway repo) | 1 | same refusal; the repo is untouched |
| installed `/Applications/Loopflow.app` lf 0.15.1 `wave list` | 0 | `No wave registry on this machine yet.` |

The last row is the Dev.app case the Task names, reproduced on the installed
release against the same store: the pre-fix reader reports an empty registry
and exits 0. The branch build refuses instead. No output said delete.

Running-reader proof: `cargo test -p loopflow --lib store::sqlite::frontier_tests::`
11 passed (67s; three SharedMachine tests ran past 60s, as the earlier
check also saw). `a_running_older_reader_survives_an_additive_upgrade_under_it`
keeps writing through an additive migration applied under it and refuses a
`-- breaks:` migration with the skew text.

## Interpretation

Every acceptance line of the Task holds on the real path: skew names the
migration and `lf install`, exits nonzero, never suggests deletion; the Task PR
wrapper names one fix. Nothing to send back to implement.

## Accepted / unresolved

- Accepted (standing in for Jack): the behavior above. No design change.
- Unresolved: none. The held counterexamples in the concept review (a step's
  own long-lived connection meeting `-- breaks:` mid-write; the credential
  lookup under skew) stay as recorded there, not reopened here.

## Next

Gate: `--test integration -- doctor_tests::status_reads`,
`--test task_pr_authority_tests`, `--lib store::sqlite::frontier_tests::`.

# Doctor observes the selected Home

Jack requested a working Doctor aligned with the current architecture and a landed PR.

Installed `lf doctor --json` reported release 0.12.29, 26 merged commits behind,
an invalid repository identity, and two missing scheduled receipts. This checkout
already contains the machine-scoped Exec identity repair. The supplied LOO-367
idle report is no longer current: `lf task status LOO-367 --json` reports running
kickoff, with provider activity. No Task recovery was performed here and the
original launch timeout cause is not established by that status.

Doctor still uses SqliteStore::new: ordinary startup admission can initialize or
migrate a store before diagnosis. Schema admission also suppresses otherwise
readable Exec evidence. This inherited the old coupling of observation to ordinary
execution admission, despite an existing read-only Exec reader for observability.
Doctor also fetches origin/main, making local diagnosis depend on network Git
operations. Installation freshness and migration compatibility are distinct.

Use read-only store inspection plus the existing Exec reader. Report missing or
incompatible storage explicitly, retain installation and scheduler checks even
when Exec reads fail, and compare only cached origin/main with that limit named.
Test missing, corrupt, and newer-frontier databases; prove no initialization or
migration and continued reporting. Preserve actual scheduled failures.

Prevention: exercise diagnosis against unavailable/incompatible storage rather
than only initialized fixtures. These changes do not repair LOO-367 or missing
cron invocations. Wave recovery remains separate.

Review finding: ordinary CLI admission initialized the database before Doctor
could inspect it. Machine Doctor now dispatches early with best-effort existing
Exec observation, retaining the one-process receipt without schema admission.
Planning Doctor keeps its existing repository path. Source-build production
Home restrictions are unchanged.

Validation: cargo test -p loopflow --test doctor_tests — 6 passed; focused Doctor unit tests — 12 passed; cargo clippy --all-targets -- -D warnings — passed; cargo fmt — passed.

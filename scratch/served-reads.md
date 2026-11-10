# LOO-790 served observations

Implementation design, 2026-11-09. Served reads and CLI subscriptions capture
external fact dependencies through `observe`; bare reads and writes observe
inline. A process-owned cache keeps answers and revisions in memory, polls only
facts already read, and is shared across requests and subscriptions. Readers
keep the existing pinned SQLite snapshot and selected-answer deduplication.
Observation intervals belong to facts, never operation names. Errors are answers
and recovery advances the revision. Unused facts expire; no schema changes.

Delete — do not maintain:
- `subscribe::Wake`, `Probe`, per-read clocks and checkout registration.
- `subscribe/checkouts.rs`, its sole `platform/fs_events.rs` dependency,
  Git's global retained-output/TTL implementation and `ask_checkouts_ahead`.
- `config::observation::Files`; use the same file facts and dependency scope.
- `SqliteStore::session_observations` and its probe-only test. Keep liveness,
  quiet-time transitions and missing checkout coverage on actual read paths.

The deletion cut is complete. Subscriptions use revision comparisons directly;
they need no stale flag. Process answers use `OsProcess` equality so activity
state changes remain visible, rather than comparing a second field list.

Acceptance: shared reads reuse observations; external Git/file/process changes
wake dependent subscriptions without store writes; unrelated facts do not rerun
a reading; argument changes replace dependencies; fresh CLI/writes bypass cache;
nested workers inherit scope and unwinding restores it; failed observations
recover. Gate also owns served_reads, subscription and direct caller suites.

Documentation: the intended-behavior edits in `docs/server.md` and
`rust/loopflow/src/server/README.md` remain in place. The documentation pass's
`lf open --host desktop --file docs/server.md --file rust/loopflow/src/server/README.md`
failed: "has no workspace in this Desktop window; nothing changed."

Gate coverage: `observe::tests::`, `lf::commands::subscribe::tests::`,
`harness::fact_writer::tests::`, `journal::request::tests::`,
`journal::os_process::tests::`, `lf::commands::top::tests::`,
`skills::catalog::`, `graph::tests::`, `store::sqlite::sessions::`,
`ops::task::tests::`, `binary(served_reads)`, `test(session_list_subscribe::)`;
include the owner-prescribed journal/agent-startup checks if gate finds that
boundary reached. Sync against main before delivery remains the auto Graph's step.

Checks (inherited LF variables unset):
- `cargo test -p loopflow --lib observe::tests::` — passed after compression (7 cache tests plus 3 matching Linear tests).
- `cargo test -p loopflow --lib lf::commands::subscribe::tests::` — passed after compression, 6 tests.
- `cargo test -p loopflow --lib harness::fact_writer::tests::served_availability_follows_the_quiet_deadline_without_a_store_write -- --exact` — passed.
- `cargo fmt` and `git diff --check` — passed; full clippy and served protocol acceptance belong to gate.
The macOS test linker warns that `__eh_frame` exceeds 16 MB; tests pass.
Compression's first stop-state regression failed on macOS: its native observer
maps live states to `S`, including stopped processes. Keep that existing platform
behavior; the stop-state regression runs on Linux in CI, with exit coverage on both.

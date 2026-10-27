# LOO-632: name the store's write-lock holder (draft, implemented)

Evidence: `scratch/database-locked-5-whys.md` (Why 1).

## Approach

- `store/contention.rs::watch_write_holds`, installed by `configure_write_connection`
  on every write connection. SQLite hooks, no call-site edits: the progress handler
  (every 1000 VM steps, reads `sqlite3_txn_state`) and the update hook start the
  clock when the connection holds the write lock. Commit/rollback hooks stop it. A hold of
  3 s or more logs `held the store write lock held_seconds=… call_site=…`, where the
  call site is the innermost `loopflow::` frame in a forced backtrace.
- `IdentifiedLines` event format (bin `init_tracing`, file log only) appends
  `pid=… lf=<BUILD_VERSION> rev=<short revision>` to every lf.log line, so every
  busy-timeout victim line (journal, activity tracker, claude pinning, prune…)
  names its process and binary without per-site edits.
- `lf doctor` check `contention` (via `contention::read_contention`): reads the last 16 MiB of lf.log and groups
  the last 24 h of `database is locked` lines and slow-writer lines by `lf (rev)`.
  Lines from older binaries group as `unidentified (older lf)`. Warn, never fail.

## Known limits

- Rust work between `BEGIN` and the first statement that ticks or changes a row
  goes unmeasured. A plain commit's WAL write isn't counted either.
- A hold reports when it ends. A writer that never commits or rolls back stays unnamed;
  its victims' identified `database is locked` lines and `lf doctor` still show the contention.
  Concept review (2026-10-26) accepted this: the incident's holds ended (2–9 s each).
- The call site depends on the symbol table. Release keeps it (`strip` is the default
  `debuginfo`). Inlined helpers report their caller.

## Ownership

`crate::diagnostic_log` owns lf.log: path, `DiagnosticLog` writer, the `IdentifiedLines`
event format (`pid= lf= rev=`, wraps the default formatter), the `field` reader and the tail
read. `store/contention.rs` keeps the write-hold hooks and `read_contention`.

## Remaining

- Demo (clone the store, run the 0.14.16 delete under lf). Belongs to the demo step.
- Demo detail: `lf doctor` reads `logs/lf.log` under `LF_HOME`, so the cloned store's
  demo runs with `LF_HOME` pointed at the clone's home.

## Checks

- `cargo test -p loopflow --lib -- contention diagnostic_log doctor` (repeated), fmt, clippy `--all-targets -D warnings`: pass after the diagnostic_log extraction and the slow-writer test repair (2026-10-26). Gate (2026-10-27): repaired `now` that the origin/main merge removed from `doctor::run`; fmt, clippy `--all-targets -D warnings` pass; focused lib 12/12, `--test integration doctor` 6/6. Full `cargo test -p loopflow` hit the 10-min limit under load from another worktree's suite; its one failure (`graph_tests::interactive_graph_keeps_input_and_advances_only_after_each_provider_exits`) passes alone (66 s) and touches no changed code. Full matrix: CI. Docs: troubleshooting + telemetry name the contention check.

## Lessons

- The slow-writer test failed ~1 in 8 runs alongside the doctor tests (12/12 alone) while its
  transaction opened with `CREATE TABLE`: DDL changes no user row and starts the clock only if a
  1000-step progress tick happens to land inside it. The test now opens with an `INSERT`. Production
  writes that open with DDL or a pure read share that documented gap.

## Delete — do not maintain

- Nothing replaced. The journal's own `waited_seconds` warn stays, and the formatter adds identity to it.

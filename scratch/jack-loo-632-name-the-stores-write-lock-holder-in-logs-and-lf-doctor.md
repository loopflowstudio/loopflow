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
- The call site depends on the symbol table. Release keeps it (`strip` is the default
  `debuginfo`). Inlined helpers report their caller.

## Remaining

- Demo (clone the store, run the 0.14.16 delete under lf). Belongs to the demo step.
- Rebase onto #1708 (doctor.rs layout) / #1705 (sqlite.rs), whichever lands first.

## Checks

- `cargo test -p loopflow --lib -- contention doctor`, fmt, clippy `--all-targets -D warnings`: pass after compress (2026-10-26). Full suites: gate.

## Delete — do not maintain

- Nothing replaced. The journal's own `waited_seconds` warn stays, and the formatter adds identity to it.

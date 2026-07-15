# Runtime store isolation

Production state must survive development from many concurrent worktrees.

## Contract

- Release binaries share `~/.lf/loopflow.db` unless the user selects another
  path.
- Checkout builds derive a development home from the embedded source worktree
  and refuse the production database without an exact break-glass opt-in.
- Tests remove and restore ambient store variables and write only to temporary
  storage.
- Session runners receive persisted control-plane binary and store state through
  dedicated `LF_CONTROL_*` variables. Provider agents lose ordinary store and
  binary variables before launch.
- Existing databases are backed up at their current migration generation before
  the same exclusive transaction advances them.
- `lf doctor` identifies the build, database, and known/applied migration
  boundary even when the database cannot be opened by the current binary.

## Proof

Tests cover ambient-state sentinels, per-worktree defaults, direct and aliased
production paths, vendor environment scrubbing, competing SQLite writers,
backup contents, and doctor output for incompatible migrations.

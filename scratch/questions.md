# Open questions and assumptions (843f5c)

Reversible choices made headless on 2026-11-05; each is one edit to change.

- **Contract markers are `-- expand: <migration>` and `-- exports: <where>`.**
  Jack's steer named the parts (an expand slug, an export) without spelling the
  header. `-- draft:` and `-- breaks:` set the `-- key: value` precedent.
- **The staging window fails hard at 30 days.** Jack wrote "about 30 days";
  `STAGING_DAYS` in `check_migrations.py` is the one place it lives and the check
  prints the date a contract clears. If "about" should allow earlier landing,
  that is a second marker, not a looser number.
- **A type change, a column made `NOT NULL`, or a new required column without a
  default fails the check outright.** The rule admits additive or contract, and
  none of these is a drop a contract can stage; the message says to expand now and
  contract later. Jack's examples covered rebuilds, triggers and drops, not these.
- **`-- exports:` is a declaration the check reads; the Rust preflight that writes
  the file is the author's.** A CI script cannot prove a hook ran; the settings
  cutover (`config::migrate::preflight`) is the named template.
- **`lf install` prints one frontier line ("advances the store from X to Y") with
  the process inventory.** Jack dropped the floor line, not the advance; it is
  visibility the inventory alone does not give.
- **The kept clone lives until the next install's commit.** Settlement keeps it
  while an older build runs; nothing later re-checks. `lf monitor prune` could, but
  that is a new behavior for another Task.
- **The missing Process row's cause is recorded as design (install dispatches
  before journal emission since #1074) plus a debug-only failure log.** The
  09:15 `lf self install --dev` row (ledger id `9b3fe9af`) went through the same
  arm; why it recorded is unknown. Install now records itself after the store
  settles, and a failed observation logs at `warn`.
- **A new unique index is reachable by an older writer unless every indexed
  column is new and its `WHERE` is not true for a row of the table's new columns'
  defaults.** The CTE binds every new column of the table, not only the indexed
  ones, so a predicate over another new column is evaluated rather than failed;
  a predicate that names an old column, or an expression index, is the break.
  SQLite's own truth decides (`CASE WHEN`), so NULL counts as outside the index,
  as it does for the real partial index.

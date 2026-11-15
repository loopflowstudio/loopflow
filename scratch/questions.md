# Open choices

- Use two disposable source roots for the initial cache comparison and one
  sequential late-root fixture for refresh; one build at a time. Admission/job budgets and shared-cache
  quiescent refresh stay outside this probe. No Machine-wide restart belongs to this probe.
- The model cut keeps application module paths as re-exports; internal callers
  import the owning crate. Public enums retain their non-exhaustive contract.
- November 15 direction supersedes the source-debugging stop. Cross-root hits
  may retain A's debug paths; use `scripts/dev-lf --isolated-build` for local
  debugging and cache diagnosis. No source-retention system is selected.
- LOO-908 is done with no PR; no live root-refresh integration is available.
  The private probe tests refresh; a shared-daemon owner remains unselected.
- The isolated build option is a leading `--isolated-build` argument to
  `scripts/dev-lf`; ordinary lf arguments pass through after it. A fixed
  checkout-local `target/isolated` is the simpler reusable debugging target.
- Storage preparation reads repository identity and saved evidence together, then
  observes configuration after the savepoint releases; this avoids a preliminary
  per-Task lookup and keeps connection reads out of SQL locks. Served observations
  retain the request's external revision tracking.
- Account selection compares its complete candidate-row/limit reading and the
  provider's newest selection at commit, rather than adding a schema revision.
  Preferences are observed again on every retry. A current empty choice needs
  only a savepoint, so it does not take the Machine's write lock.
- Direct ensure of a retired Wave address writes the fresh identity to authored
  config; discovery still skips retired subtrees. Keeping the retired ID in that
  file would mint another identity after a SQL rollback, so the fresh ID is retained.
- Saved records are placed in the storage crate unless a storage-independent
  crate needs them; the alternative (all saved records in `loopflow-model`)
  widens every record edit's rebuild to the terminal. Steer markers count as
  storage's own files and keep their touch inside the write transaction.
- After the storage crate moves, measure before choosing the next cut: `ops` and
  `lf` (about 123k of 331k lines) stay in the application either way.
- Capacity commands retain the configuration editor's file lock through the SQL
  command. This serializes declaration removal with admission without extending a
  SQL transaction across file acquisition; the same file editor handles ordinary
  preference updates.
- The upgrade preflights follow the read/act/compare seam (storage returns the
  export and process rows, the application writes files and judges processes,
  storage compares inside the exclusive transaction). The alternative, a
  callback run inside the transaction, is less code but puts application code
  under storage's lock and contradicts the no-callback direction.
- Rename, bind and attach check the Session caller whatever the title source;
  title repair checks none. This newly refuses a replaced provider binding or
  attaching a Session a person named, and lets a Task identifier update repair
  the editor's own replaced Session. The alternative keeps today's behavior
  exactly (check only when the title is generated, skip repair for a stale
  editor), which needs the caller threaded through every planning write.
- Acquisition modules under `store/` keep their paths until the storage
  measurement picks the next cut; renaming them now changes no rebuild closure.
- November 15 disposable cutover probe shows a changed SQL export cannot retry
  through the existing journal. The upgrade move returns to review; the draft
  journal rollback/archive sequence and crash checks are in `loo-650.md`.

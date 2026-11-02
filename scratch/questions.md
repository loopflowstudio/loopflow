# LOO-693 plan: running steps and conversations survive a store upgrade

Reconciled 2026-11-02 against HEAD `677ffca2b7`; origin/main has not moved
past the base `d30b3a8ca`.

## Exists (proved on this branch)

- **A running older reader survives an additive upgrade.** The Graph runner
  revalidates its store after every step (`graph.rs`, `validate_current_schema`);
  `frontier_tests::a_running_older_reader_survives_an_additive_upgrade_under_it`
  applies an additive slug migration under an open reader and keeps writing,
  then applies a `-- breaks:` migration and asserts the refusal names the
  skew, ends with ``run `lf install` `` and never says delete.
- **Skew surfaces instead of reading as an empty registry.** `open_existing_store`
  returns `StoreResult<Option<Store>>`: `Ok(None)` only when no store file
  exists; an unopenable store is an error. `doctor_tests::status_reads_name_a_newer_store_instead_of_reading_empty`
  runs `wave status`, `wave status --json`, `wave list` and `task status`
  against a store with a future migration and asserts nonzero exit, the skew
  and fix on stderr, nothing on stdout. This is the Loopflow Dev.app case.
- **No message suggests deleting the database.** `incompatible()` says keep
  loopflow.db and open it with the migrating build, or run `lf install`.
  The `delete loopflow.db` text the Task cites was already gone at the base.
- MIGRATIONS.md already documents "Older lf keeps opening an additively
  extended store"; no doc change needed.

## Remains

Nothing in this Task's scope. Gate owns the affected suites.

## Assumptions (Jack Heart's decisions of 2026-11-02 applied; revisit if wrong)

- **Skew now fails launches that used to degrade.** Callers that treated a
  refused store as "no registry" (implicit Work binding on launch, `lf context`,
  session resume over ssh, `lf ci`, metrics publishing) now exit nonzero with
  the skew and `lf install`. An agent reading "nothing here" as fact is the
  bug; a launch path that should keep running unbound under skew would be a
  product decision, left open.
- **One registry opener.** `open_registry_for_authority` and `RegistryUnavailable`
  are deleted; their seven callers use `open_existing_store`. Refusal messages
  name the path only where a caller still adds `default_db_path()`.
- **`storage_config_from_env`/`database_path_from_env` cannot fail** yet return
  `Result`; 89 call sites in 45 files map an impossible error. Making them
  infallible is its own Task.
- **Private `LF_HOME` stores still refuse any newer migration**, additive or
  not: they self-migrate (`apply_sqlite_with_backup`). Only the shared `~/.lf`
  store admits additive newer tails, which is where running steps live.

Check: `cargo test -p loopflow --lib store::sqlite::frontier_tests::` 11 passed;
`--test task_pr_authority_tests` 12 passed; `--test integration -- doctor_tests::status_reads` passed;
clippy clean (tree unchanged since; realign reran nothing).

# LOO-693 open questions and assumptions

- **Skew now fails launches that used to degrade.** `open_existing_store` returns
  `StoreResult<Option<Store>>`: `None` only when no store file exists. Callers
  that silently treated a refused store as "no registry" (implicit Work binding
  on launch, `lf context`, session resume over ssh, `lf ci`, metrics publishing)
  now exit nonzero with the skew and `lf install`. Chosen because an agent
  reading "nothing here" as fact is the bug; revisit if a launch path should
  keep running unbound under skew.
- **One registry opener.** `open_registry_for_authority` and `RegistryUnavailable`
  are gone; their seven callers read `open_existing_store` (`Ok(None)` = no
  registry, `Err` = present but unopenable). Refusal messages lost the path
  except where a caller still names it via `default_db_path()`.
- **`storage_config_from_env`/`database_path_from_env` cannot fail** but return
  `Result`, so 89 call sites in 45 files map an impossible error. Making them
  infallible is its own Task.
- **Private `LF_HOME` stores still refuse any newer migration**, additive or not:
  they take the self-migrating path (`apply_sqlite_with_backup`). Only the
  shared `~/.lf` store admits additive newer tails, which is where running
  steps live. Left as is.
- The `delete loopflow.db` advice the Task cites (migrations.rs:131) was already
  gone at d30b3a8ca; `incompatible()` now says keep loopflow.db. Both new tests
  assert no refusal mentions deleting.

Checks: `cargo test -p loopflow --lib store::sqlite::frontier_tests::` 11 passed;
`cargo test -p loopflow --test task_pr_authority_tests` 12 passed;
`cargo test -p loopflow --test integration -- doctor_tests::status_reads` passed;
clippy clean. Gate owns affected suites.

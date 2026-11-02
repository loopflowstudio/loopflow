# LOO-693 open questions and assumptions

- **Skew now fails launches that used to degrade.** `open_existing_store` returns
  `StoreResult<Option<Store>>`: `None` only when no store file exists. Callers
  that silently treated a refused store as "no registry" (implicit Work binding
  on launch, `lf context`, session resume over ssh, `lf ci`, metrics publishing)
  now exit nonzero with the skew and `lf install`. Chosen because an agent
  reading "nothing here" as fact is the bug; revisit if a launch path should
  keep running unbound under skew.
- **Two "open the registry, keep the reason" helpers remain.**
  `open_registry_for_authority` (`RegistryUnavailable`) now overlaps
  `open_existing_store`; its `Unresolved` arm is unreachable because
  `database_path_from_env` never fails. Folding its seven callers into
  `open_existing_store` is a follow-up reduction, left out to keep this diff on
  the skew behavior.
- **Private `LF_HOME` stores still refuse any newer migration**, additive or not:
  they take the self-migrating path (`apply_sqlite_with_backup`). Only the
  shared `~/.lf` store admits additive newer tails, which is where running
  steps live. Left as is.
- The `delete loopflow.db` advice the Task cites (migrations.rs:131) was already
  gone at d30b3a8ca; `incompatible()` now says keep loopflow.db. Both new tests
  assert no refusal mentions deleting.

Checks: `cargo test -p loopflow --lib store::sqlite::frontier_tests::` 11 passed;
`cargo test -p loopflow --test integration doctor_tests::status_reads` passed;
clippy clean. Gate owns affected suites.

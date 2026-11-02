# LOO-693 plan: running steps and conversations survive a store upgrade

Reconciled 2026-11-02 against HEAD `2c2e1fb669`. Code last changed in
`7730f178c6` (compress: test trim and one doc comment). origin/main
(`7404ba92ef`, #1792) is merged; it shares no file with this change.

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
  bug. Settled by the concept review below: no launch path keeps running
  unbound under skew.
- **One registry opener.** `open_registry_for_authority` and `RegistryUnavailable`
  are deleted; their seven callers use `open_existing_store`. Refusal messages
  name the path only where a caller still adds `default_db_path()`.
- **`storage_config_from_env`/`database_path_from_env` cannot fail** yet return
  `Result`; 89 call sites in 45 files map an impossible error. Making them
  infallible is its own Task.
- **Private `LF_HOME` stores still refuse any newer migration**, additive or
  not: they self-migrate (`apply_sqlite_with_backup`). Only the shared `~/.lf`
  store admits additive newer tails, which is where running steps live.

Check: `cargo test -p loopflow --lib store::sqlite::frontier_tests::` 11 passed after the compress trim; `--test task_pr_authority_tests` 12 passed; `--test integration -- doctor_tests::status_reads` passed; clippy clean; realign (prose only) reran nothing; gate owns the affected suites.

## Concept review (2026-11-02, headless)

Reviewed the implemented change against Jack Heart's decisions of 2026-11-02.
Page: `scratch/concept-review.html`.

**Accepted experience.** Jack installs a newer lf while steps and conversations
run on the older one. Nothing he does changes: the install migrates `~/.lf`,
running work keeps going, and any lf that can no longer read the store says
which migration it lacks and `run \`lf install\``, exiting nonzero. Agents never
see an empty registry. Nobody is told to delete loopflow.db.

**Model.** Two concepts carry it. A *store* has a migration frontier; a
*reader* is any lf process holding it open. A newer tail is additive unless a
migration says `-- breaks:`. Skew is a reader behind a breaking tail. Three
surfaces: `open_existing_store` (one opener; `Ok(None)` only when no file),
`validate_current_schema` after every Graph step, and the two refusal texts
(`this store needs an lf that knows <v> … run \`lf install\`` for a breaking
tail; `incompatible()` for a diverged history). No new record, flag or state.

**Settled from accepted decisions.**
- Launch under skew fails with the skew. "Exit nonzero and name the fix"
  covers launches; one lf per Mac (LOO-669/688) makes skew itself the defect,
  and an unbound launch would hide it from the one person who can run
  `lf install`. The earlier "left open" note is withdrawn.
- `Ok(None)` keeps meaning "no store on this Machine yet"; callers that say
  `run \`lf doctor\`` for that case are right, since there is no skew to name.

**Normal path, failure, recovery.**
- Additive install under a running Graph: the step finishes, revalidation
  passes, the Graph continues. Proved by `frontier_tests`.
- Breaking install under a running Graph: the step's row is recorded, then
  the Graph stops naming the skew and `lf install`; the worktree keeps the
  step's files. `lf install`, then `lf task run <ID>` restarts that run from
  where it began (the ordinary stopped-Graph contract, not a replay of effects).
- Held counterexample, no change: a step's own long-lived connection that
  meets a `-- breaks:` migration mid-write fails with a raw SQLite error, not
  the skew text. Every fresh lf the agent runs inside the step, and the Graph
  at the step boundary, name the skew. Wrapping every write in a revalidate
  would be machinery for a case `-- breaks:` is designed to make rare.
- Private `LF_HOME` stores refuse any newer tail because they self-migrate;
  running steps live in `~/.lf`, so the rule stays. Developers sharing one
  private store between two builds see the refusal with `lf install`.

**Open product choices.** None.

**Next proof.** Gate: the suites named under Check above; nothing new.

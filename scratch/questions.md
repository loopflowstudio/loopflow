# LOO-693 plan: running steps and conversations survive a store upgrade

Reconciled 2026-11-02 after implement and compress landed the concept
review's one finding (`040c41ab79`, `cf6f2a0c6e`). origin/main (`d404104d55`,
#1777) is merged; it touches only the Wave memory, merged without conflict.

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
- **The Task PR refusal names one fix.** `authority_store`
  (`rust/loopflow/src/ops/task/delivery.rs`) is the one open both Task PR
  entry points use; it prefixes `Task PR authority refused: the shared
  Loopflow registry <path> cannot be opened:` and passes the store's error
  through verbatim, as planning, linear, context and session_move do; under
  skew that error already ends in ``run `lf install` ``.
  `missing_task_registry` is unchanged: no store is not skew, and `lf doctor`
  is right there. `task_pr_authority_tests` asserts the store's own text
  (`not a database`, `unable to open database file`) for the corrupt and
  inaccessible registries. Settled by the concept review of 2026-11-02
  (second pass) below.

## Remains

Nothing.

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

Check (gate, 2026-11-02): `uv run python scripts/test.py --reuse-passing --rust-filter 'test(store::) | test(doctor_tests::) | binary(task_pr_authority_tests) | test(context_tests::) | test(wt_prune_branches::) | test(planning_setup_tests::) | test(machine_commands::) | test(work_open_tests::) | test(wave_setup_tests::)'` PASS: architecture, inventory, fmt, 501 Rust tests passed (one pre-existing SharedMachine fixture over 60 s, `task_work::tests::process_membership_does_not_read_events_that_name_no_process`), clippy clean; 623 s. CI owns the full Rust matrix.

## Concept review (2026-11-02, headless; third pass after the one-fix refusal landed)

Reviewed the implemented change against Jack Heart's decisions of 2026-11-02.
Page: `scratch/concept-review.html`. The second pass found the Task PR
wrapper's second fix, now landed (`040c41ab79`, `cf6f2a0c6e`). The third pass
audited every `open_existing_store` caller against "name one fix": each
prefixes who refused and passes the store's error through; none appends a
fix after it. Nothing to change; no open choice.

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
- A refusal names one fix. The store's error carries ``run `lf install` `` when
  the cause is skew; a caller that appends its own fix after it (`lf pr submit`
  from a Task worktree did, before this change) contradicts "name the fix" and
  sends the operator to `lf doctor`, which cannot install anything. Callers
  prefix who refused and the path, then pass the store error through verbatim.

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
- Held counterexample, no change: the stored-credential lookup at agent
  launch (`apply_provider_env_to_command`) warns and injects nothing when the
  store cannot be opened. Under skew it is unreachable for Work launches:
  `implicit_binding` (`run.rs`) and the selector path (`bin/lf.rs`) propagate
  the store error first, so the launch exits with the skew before credentials
  are read. Its other callers are `lf auth` flows, not launches of Work.
  Making it fail would add a second refusal on a path that already refused.
- Private `LF_HOME` stores refuse any newer tail because they self-migrate;
  running steps live in `~/.lf`, so the rule stays. Developers sharing one
  private store between two builds see the refusal with `lf install`.

**Open product choices.** None.

**Next proof.** Gate: the suites named under Check above; nothing new.

# Open choices

- November 15 resumed concept review: name the lower compiler unit
  `loopflow-execution`; keep command semantics and running-build identity as
  application inputs, with no saved Graph format change or generic adapter
  service framework. This reversible implementation choice follows the existing
  behavior-preserving cut. No product question is open; iterate to implement.

- November 15 concept review: complete the existing engine/domain operation
  closure below CLI and server dispatch. Move native resume and the Machine
  client with their execution owners; keep Clap catalogue acquisition above and
  pass typed catalogue values to shared listing. This reversible design direction
  changes no commands, receipts or execution policy; no product question is open.

- November 15 implementation: the selected cache upkeep entry is `scripts/dev-lf`
  before Cargo. Configuration reload follows the existing server's natural idle
  exit; no shared-daemon restart or new schedule. Ordinary Cargo does not refresh
  inventory, explicit `SCCACHE_CONF` remains caller-owned, and isolated builds
  bypass it. This reversible placement does not install Machine configuration.
- The execution crate is extracted; independent fixture/behavior verification
  and distinct command/execution/saved-record rebuild receipts remain required.

- Use two disposable source roots for the initial cache comparison and one
  sequential late-root fixture for refresh; one build at a time. Admission/job budgets and shared-cache
  quiescent refresh stay outside this probe. No Machine-wide restart belongs to this probe.
- The model cut keeps application module paths as re-exports; internal callers
  import the owning crate. Public enums retain their non-exhaustive contract.
- November 15 direction supersedes the source-debugging stop. Cross-root hits
  may retain A's debug paths; use `scripts/dev-lf --isolated-build` for local
  debugging and cache diagnosis. No source-retention system is selected.
- LOO-908 is done with no PR. The new developer-entry inventory owner is local to
  this Task; the private root probe checks natural idle reload without a restart RPC.
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
- Capacity commands retain the configuration editor's file lock through the SQL
  command. This serializes declaration removal with admission without extending a
  SQL transaction across file acquisition; the same file editor handles ordinary
  preference updates.
- Upgrade preflights stay inside the exclusive transaction and reach outside
  through closures the application passes at open; the settings exporter keeps
  the transaction's connection. Read/act/compare for upgrades is withdrawn: it
  needed a new changed-source recovery to restore what the held lock already
  gives. The alternative kept open is storage returning old-schema rows as
  values, which removes the application's raw connection at the cost of
  rewriting the released exporter.
- The changed-source refusal after an interrupted cutover is existing behavior
  and stays; its drafted recovery is outside this Task.
- Rename, bind and attach check the Session caller whatever the title source;
  title repair checks none. This newly refuses a replaced provider binding or
  attaching a Session a person named, and lets a Task identifier update repair
  the editor's own replaced Session. The alternative keeps today's behavior
  exactly (check only when the title is generated, skip repair for a stale
  editor), which needs the caller threaded through every planning write.
- Acquisition modules under `store/` keep their paths until the storage
  measurement picks the next cut; renaming them now changes no rebuild closure.
- The token key reaches storage as a lazy source supplied at open; the check for
  existing encrypted credentials stays against the Machine home. The alternative
  moves `lf_home_dir` and key acquisition down with storage as "its own files",
  which puts ambient home selection back under the opener that just lost it.
- A Task's captured Graph and cursor reach SQL as saved JSON text and decode in
  `store::task_work`; addresses and the display Graph move down as types. The
  alternative moves `CapturedGraph` down with `ConcreteStep`, `Skill`,
  occurrence settings and `Command`, so a skill-setting edit rebuilds storage.
- Repository writers accept historical raw spellings. Scoped Task lookup now
  observes candidate canonical aliases above SQL and compares its complete saved
  candidate rows in the final savepoint. Capture-history projection follows the
  concept-review cut in `loo-650.md`: SQL selects and reads one input's evidence;
  the application validates the full manifest and projects it. Keep activity's
  early stop before evidence loading; no eager page or partial manifest decoder.

- Mixed child fixtures use observed peer import above storage; prepared SQL tests
  keep local writer/title repair and Task-field persistence below it. This avoids
  exporting a transaction helper solely for the application fixtures.

- Unknown future provider kinds use the existing missing-token refresh error; no
  provider command is attempted. Known non-refreshing kinds retain that behavior.
- The demo World records upstream disposition through a storage-owned command;
  the hook's quiet cache reads the database path through storage. Raw connection
  access is available only with the dev-only `test-support` feature.

- The embedded empty test database uses a fixed build-time Machine placeholder;
  each home copy still generates its own identity and timestamps. This keeps
  test-support library bytes reusable across roots without sharing Machine identity.

- Historical SQL fixtures belong to storage. The canonical helper consumes a
  generated SQL literal, anchored by its build script, to keep test-support
  reuse independent of application files and manifest-root environment values.

- The clean-to-dirty lower-crate provenance check now passes with the production
  build script and real Cargo. It is independent of the older sequential
  shipping/Task-record timing cases.

- November 15 native/Machine cut: use a generic protocol envelope instantiated
  with typed CLI operations only at dispatch. Native clients serialize their own
  typed operation values, and the peer keeps validation. Wire fields/revision,
  native provider byte paths and readiness remain unchanged. This removes the
  client dependency on `Read`/`Write` without a second protocol implementation.
- November 15 catalogue cut: retain the existing complete `Entry` value as
  the typed input. Shared discovery supplies definitions and Waves; the adapter
  derives invocation spellings and command entries from Clap, then the shared
  operation merges and sorts. No copied navigation tree or callback service.
- Landing-command semantics currently depend on full Clap normalization. Keep
  alias/abbreviation/global-flag behavior when supplying typed catalogue values;
  no prefix-based command classifier is selected. Application provenance must
  enter the reusable execution library as an input, not as compiled root bytes.

- November 15 delivery observation: `lf task hold LOO-917` stopped Graph
  `9b4c441a-9a09-40ce-889b-0d56276bd191` and disabled auto-merge, but left its
  native `land --wait-and-fix` step `3ae30c73-d6a2-4ac3-a10f-379b425d0717`
  running as orphan PID 67960 (start 1794795959, foundation checkout cwd). It
  prevented the next ship with “live or unresolved execution”. The exact
  recorded PID/start/cwd/argv matched before SIGTERM; it exited 130, and the
  next ship started. This runtime-control defect is outside the crate cut.
  Cheapest regression producer: hold a disposable Task whose Graph command
  waits, then assert its recorded command process exits and a new ship starts.

- November 15: a broad compiler-process inspection accidentally printed an unrelated Cursor API credential from process arguments. The value was not used or copied; it needs rotation. Further inspection is restricted to known PIDs.

- November 15 measurement: five fresh targets per revision, caches seeded once,
  paired order alternates candidate/base then base/candidate. Keep each semantic
  edit unique to its cohort; report instrumented client CPU/RSS and include links.
  Root-probe cache stays private; no Machine configuration change.

- November 15 fixture revision: dev-feature unification must not turn a compiled
  CLI into a unit-test runtime. Keep unit-test defaults at `cfg(test)`; the
  inspection feature exposes explicit hooks only. The remote recovery run
  showed a Process/Session ledger split before movement, recorded in the plan.

# LOO-955: one tracker vocabulary

The glossary in `docs/dev/planning.md` § Words (from #1957) is the one
definition; this change makes code, config, CLI, wire and docs say it. Its
"Older code still says…" paragraph is deleted when this lands.

| Word | Replaces |
|---|---|
| plan | a Wave's "plan" meaning its current Project (`edit-plan`, `update-plan`) |
| tracker | planning provider, bridge, `pm_` |
| action | effect, receipt, delivery, export, settle (outbound to a tracker) |
| event | observation, acceptance, acquisition (inbound from a tracker) |
| Project | Chapter |
| provider | only the agent's provider (claude, codex) |

**Scope rule.** A word is renamed only where it means the tracker concept.
`observe.rs` / `observe::` (external facts in served reads), metric
observations, steer/PR/notification deliveries, Swift `*Observation` view
services (`WorkObservation`, `LocalLineObservation`…) and creation receipts
keep their names: they are not tracker events or actions.

## What people see

```
$ lf repo tracker                 # was `lf repo plan`
tracker  linear (team 3871bed0)
open     2 actions   (1 sent, 1 queued)
lost     1           lf repo tracker answer 9f2c41d0 --keep-theirs | --use-mine
$ lf repo tracker key | journal | answer <id>
```

```yaml
# .lf/config.yaml, new spelling (read now, written once a release reads it)
tracker:
  linear: {team: 3871bed0-…}
# also: tracker: {git: {remote: origin, shared: main}}, tracker: {asana: {team: …}}, tracker: git
```

```
$ lf wave edit-project product --name "2026-10 — customer work"   # was edit-plan
$ lf wave update-project --wave product --file plan.json            # was update-plan --plan
$ lf wave plan product          # unchanged: the Wave's Projects and Tasks, the plan
```

`lf wave …` help, `lf wave status` text and errors say Project, never chapter.
`lf task show --json` / Work DTOs say `tracker_id` and `tracker_state`.

## Done when

- The chapter grep excludes released migrations and fixtures, release notes and authored Wave files. It also finds the released lock directory shared by concurrent writers, historical directory exclusions and this scratch plan. The Linux OAuth experiment still assumes a retired schema and CLI; see `scratch/questions.md`.
- `provider` never names a tracker in `rust/**/{linear,asana,ops,store,config}`, `docs/`, CLI help, except the kept column names (`planning_provider_revision`) and the released config tag `planning.provider`.
- `edit-plan` / `update-plan` / `WavePlanTarget` are gone; `docs/dev/planning.md` § Words and AGENTS.md no longer call the current Project "the plan".
- `git grep -E 'TrackerObservation|linear_acceptance|Acquisition|pm_(items|projects|issue_changes|wave_projects|wave_sync)'` outside migrations, historical upgrade fixtures, and compatibility tests for released writers is empty.
- `lf repo tracker` (and `key`, `journal`, `answer`) works; `lf repo plan` is gone; Desktop's sync view calls the new path; `documented_commands` passes.
- A repo with `tracker: {linear: {team: X}}` and a repo with released `planning: {provider: linear}` + `linear: {team: X}` resolve to the same `TrackerConfig`; `lf config set` still writes the released shape.

## Acceptance check at gate

```bash
cargo test -p loopflow-execution --lib config::keys::           # both spellings fold to one TrackerConfig
cargo test -p loopflow --test integration documented_commands:: # docs name lf repo tracker
cargo test -p loopflow-storage tracker_table_mirror              # old lf writes pm_items, new lf reads tracker_items and back
uv run python scripts/check_migrations.py                       # expand is additive
swift test --filter DTOFixtureTests                              # tracker_id / tracker_state round-trip
```

## Decisions

### Config: `tracker:` is a read spelling; the writer keeps `planning:` (made for review)

`config/keys.rs::fold` already folds `repo.plan`, `plan`, `linear.team` into the
typed `planning` key. Add `tracker` (and canonical `repo.tracker`) to the same
fold: an externally tagged map `{linear|asana|git: {...}}` or a bare scalar
becomes `planning: {provider: <tag>, ...}` before typed deserialization. The
typed/serialized layout stays the released `planning: {provider}` because the
released lf reads `.lf/config.yaml`; mark the `RawPlanningConfig` tag and the
fold with `// Released shape until LOO-784 writes canonical spellings`.
LOO-784 ("Write repo.plan and the other canonical config spellings once a
release reads them") already owns flipping the writer; steer it to include
`tracker:` and to delete the `planning`/`provider` spelling. No new Task.

Rust names change now: `PlanningConfig` → `TrackerConfig`, `RawPlanningConfig`
→ `RawTrackerConfig`, `GitPlan` stays (it names which plan ref). This repo's
own `.lf/config.yaml` stays on the released shape.

`repo.tracker` is the canonical key and `tracker` its shorthand, like
`repo.plan`/`plan` today. `repo.plan` and `plan` were documented in v0.16.0
and name the tracker "plan" with a `provider` tag; they stay read-only folds
under the same LOO-784 marker, and every doc and help string teaches only
`tracker`. Dotted keys follow the enum: `lf config set repo.tracker.linear.team X`
writes `planning: {provider: linear, team: X}`; naming a different tracker
replaces the block (a repository has one tracker), never merges two tags.
`typed()` gains that one rule; `lf config get repo.tracker` prints the
canonical shape.

Passed over: `tracker: linear` + sibling `linear: {team}` — reuses the released
block but splits one value across two keys and collides with Wave files'
`linear: {initiative}`.

### `lf repo plan` → `lf repo tracker`

It shows the tracker, its open and lost actions and Git's destinations.
`lf repo sync` was the other candidate; it reads as a verb that does
something. No alias (solo dev, Desktop ships with lf). Callers:
`rust/loopflow/src/lf/mod.rs` (`RepoPlanCommand` → `RepoTrackerCommand`),
`swift/Loopflow/Services/RegistryQuery.swift:182,424`,
`swift/LoopflowTests/DesktopHeadlessTests.swift:77`, `docs/work.md`,
`docs/surfaces.md`, `docs/dev/planning.md`, `ops/README.md`, user-facing
strings in `ops/planning_sync.rs` and `ops/task.rs`.

### A Wave's current Project is a Project, not its plan

The plan (glossary) is the shared Waves, Projects and Tasks; `lf wave plan`
prints a Wave's part of it and keeps its name, with `bind`, `connect`,
`reteam` (they bind that plan to the tracker). The two commands that change
the current Project rename: `WaveCommand::EditPlan` → `EditProject`
(`lf wave edit-project`), `UpdatePlan` → `UpdateProject`
(`lf wave update-project --file`), `WavePlanTarget` → `WaveProjectTarget`.
Callers: `docs/work.md:434,442`, `docs/dev/planning.md:9,400-405`,
`docs/dev/reference.md:84`, builtin `split-wave.md:36-40`. Glossary rows
become "plan | the shared Waves, Projects and Tasks" and "Project | a Wave's
current Project: its KRs, targets and Tasks"; AGENTS.md "A Wave's current
plan is its In Progress Linear Project" says "A Wave's current Project is its
In Progress Linear Project". No alias.

### Inbound is event

`TrackerObservation` → `TrackerEvent`; `observe_issue` → `fetch_issue`
(a request, the result is the event); `linear/acceptance.rs` →
`linear/events.rs`; `sqlite/linear_acceptance.rs` → `sqlite/linear_events.rs`;
`linear/observations.rs`, `sqlite/linear_observations.rs`,
`store/linear_observation_tests.rs` → `*_events*`; `TaskAcquisition` →
`TrackerState`, `LinearIssueEvidence` → `LinearIssueState` (the last event
kept per issue; `TrackerState` is its per-Task projection, so the pair says
one thing at two grains).
`task_events` / `project_events` keep their names: local history, and the
`tracker_` prefix keeps the two apart.
`ops/tracker_events.rs` already uses the word.

### Wire: fold LOO-942 in

Task and Project DTOs: `linear_id` → `tracker_id` (Asana gids fill it too),
`acquisition: {state, observed_at}` → `tracker_state: {state, synced_at}`.
Changes with the Swift mirror (`WaveWorkMap.swift`, mocks, tests) and
`tests/fixtures/dto/*` in the same slice; `server_schema.json` regenerates.
`MetricContractIssueDto::ChapterUnavailable` (`"chapter_unavailable"`, decoded
in `MetricPortfolio.swift:255`) → `ProjectUnavailable`, and
`WaveRemoval.chapters` → `projects`, each with its Swift mirror. Close LOO-942
as folded in when the PR publishes.

### Tables: `pm_*` and `planning_peer_*` renamed with mirror triggers (staged)

A rename drops the old table, so `check_migrations.py` requires staging, and a
released lf keeps writing the old name for the 30-day window. Probe
(sqlite3, 2026-11-17): two real tables with AFTER INSERT/UPDATE/DELETE
triggers each way stay identical under plain insert, `ON CONFLICT DO UPDATE`,
`INSERT OR REPLACE`, cross-table update and delete; with
`recursive_triggers` off (the store never sets it) the pair does not loop.
Views with INSTEAD OF triggers were rejected: old code upserts
(`planning_peers.rs:147,391,573`) and SQLite refuses UPSERT on a view.

Expand migration `tracker_tables.sql` (one per Task): create each new table
with the same columns, copy rows, add the six mirror triggers and the
`store_revision_*` triggers on the new name. Runtime SQL uses only new names.
The contract (drop old tables and mirrors, plus dead `pm_snapshots`,
`pm_project_name_cutover`) is a new Task dated 30 days after merge.

| Old | New |
|---|---|
| `pm_items` | `tracker_items` |
| `pm_projects` | `tracker_projects` |
| `pm_issue_changes` | `tracker_events` |
| `pm_wave_projects` | `tracker_wave_projects` |
| `pm_wave_sync` | `tracker_wave_sync` |
| `planning_peer_{routes,changes,conflicts,imports,observed}` | `git_tracker_{routes,changes,conflicts,imports,seen}` |
| `planning_destinations` | `git_tracker_destinations` |

Kept: `planning_user` (the person's key in the plan), `linear_team_checks`
(Linear-specific, correctly named), `tracker_actions`. Columns
(`planning_provider_revision`, `pm_snapshot_synced_at`, `planning_*` on
`tasks`/`projects`) are out: column renames on the two hottest tables cost a
rebuild each and are noted in `scratch/questions.md`.

### Git tracker: destination, not peer (made for review)

The tables say `git_tracker_*` and `fetch_error`; Rust, the wire and Swift still
say peer and acquisition. One name each: the Git tracker is the mechanism, a
destination is one ref it exchanges with.

| Old | New |
|---|---|
| `PeerPlanningStatus` | `GitDestination` |
| `PeerProjectionConflict` | `GitDestinationConflict` |
| field and wire key `acquisition_error` | `fetch_error` |
| `loopflow-storage/src/peer_records.rs` | `git_tracker.rs` |
| `sqlite/planning_peers.rs`, `store/planning_peers.rs` | `sqlite/git_tracker.rs`, `store/git_tracker.rs` |
| `planning_sync::peer_status`, `acquire_repository` | `git_destinations`, `fetch_repository` |
| Swift `PeerPlanning.swift`, `PeerPlanningView.swift` and their types | `GitDestination.swift`, `GitDestinationsView.swift` |

`planning_git.rs` keeps its name (Git transport for the plan). Swift
`TrackerSync*` already holds actions and is not reused. The mirror triggers
already map the released `acquisition_error` column, so no SQL changes.
`docs/dev/planning.md` § Words gains: "destination | one Git ref the plan is
exchanged with, listed by `lf repo tracker`". Passed over: keeping "peer" in
code with only the wire key renamed.

### Task and Project JSON: no `pm_` keys (made for review)

Struct fields and JSON keys `pm_snapshot_synced_at` → `tracker_synced_at`,
`pm_writeback` → `tracker_writeback` (`LinearWritebackState` keeps its name);
`lf task status` prints `tracker writeback:`. SQL column names stay. Passed
over: a serde rename alone (field and wire would disagree), and dropping
`pm_snapshot_synced_at` for `tracker_state.synced_at` (needs every writer of
both read first).

### The explanation's sync time is `tracker_synced_at` (made for review)

Task explanations read `tracker_synced_at` directly from the Task and print
`Tracker synced:`. Swift decodes the required nullable key as `trackerSyncedAt`.
The tracker lookup functions are `fetch_task` and `fetch_from_tracker`.

### The explanation's sentences say fetch and tracker (made for review)

`lf --explain task …` prints prose around `Tracker synced:` that still says
acquisition and Provider. Exact text:

| Where | Now | Becomes |
|---|---|---|
| `ops/task/planning_explain.rs:127` | `Nothing was saved, acquired or synchronized.` | `Nothing was saved, fetched or synchronized.` |
| `:283` | `Provider freshness, synchronization delivery and concurrent saves are not observed or reserved` | `Tracker freshness, queued tracker actions and concurrent saves are not read or reserved` |
| `:296` | `{reason}; no planning acquisition was performed` | `{reason}; nothing was fetched from the tracker` |
| `:299` | `No saved Task selected; no planning acquisition was performed` | `No saved Task selected; nothing was fetched from the tracker` |
| `:361` | `{error}; Wave registration/acquisition was not performed` | `{error}; the Wave was not registered and nothing was fetched from the tracker` |
| `ops/task.rs:1919` (log) | `planning acquisition after creation failed` | `tracker fetch after creation failed` |

"Observation only;" opens three explain reports and means the explanation
itself; it stays under the scope rule, as does the `Effect:` label. Passed
over: leaving the sentences as prose (they are what a person reads when the
command fails).

`planning_write`, `planning_order`, `planning_reads`, `planning_sync` stay:
they are the plan writer and sync.

## Slices

1. **Tables (implemented).** `tracker_tables.sql`,
   runtime SQL switched, one storage test that writes through each old name and
   reads the new (and back), `check_migrations.py` green, `EXPLAIN QUERY PLAN`
   on the renamed hot reads (`query_plans.rs`). Shows whether mirror triggers
   coexist with the canonical-identity and revision triggers.
2. **CLI + config (implemented).** CLI and Desktop use `repo tracker` and its
   served routes. Wave Project edits use `edit-project` and `update-project --file`.
   Config reads both tracker spellings, writes the released layout and
   replaces the selected tracker on a dotted edit. LOO-784 carries the writer cutover.
3. **Wire + Project vocabulary (implemented).** Task status and Work share `tracker_state {state, synced_at}`. Tasks and Projects expose `tracker_id`; metric issues use `project_unavailable`, and Wave removal counts `projects`. Rust, Swift, CLI text, the server schema and shared fixtures match. Stored creation receipts keep the binding readable by released writers.
4. **Events and tracker prose (implemented).** Linear and Asana produce `TrackerEvent`. Task steering keeps a per-Task cursor, and planning reads the latest `LinearIssueState`. CLI help, Desktop comment conflicts and planning docs name the tracker. LOO-960 owns the contract migration 30 days after merge.

5. **Git destination and Task JSON names (implemented).** Rust, Swift and the
   server schema use `GitDestination`, `GitDestinationConflict` and
   `fetch_error`. Task and Project JSON expose `tracker_synced_at`, and the
   Task's writeback field is `tracker_writeback`. Released names remain in
   stored creation receipts for replay across CLI versions, historical
   migration references, SQL columns and mirror tests. The glossary defines
   destination.

6. **Explanation sync time and fetch verbs (implemented).** Rust, Swift and
   the shared explanations use the Task's sync timestamp. A Task that has
   never synced reports null in JSON and unknown in the CLI.

7. **Explanation sentences (implemented).** Task previews and the post-creation
   warning use the fetch and tracker wording above. The three focused CLI tests
   passed, the retired wording grep found no matches in Rust or docs, and
   `cargo clippy --all-targets -- -D warnings` passed. Gate follows.

## What goes

`lf repo plan` and `RepoPlanCommand`; `lf wave edit-plan`, `update-plan`, `WavePlanTarget`; `repo.plan` in docs and help; the "Older code still says…" paragraph;
`TrackerObservation`, `TaskAcquisition`, `PeerPlanningStatus`, `PeerProjectionConflict`; DTO keys `linear_id`, `acquisition`, `acquisition_error`, `pm_snapshot_synced_at`, `pm_writeback`,
`chapter_unavailable`, `chapters`; every runtime reference to the old table
names. Released migrations, release notes and released-schema fixtures under
`tests/fixtures/` keep old names (shipped records).

## Checks

- Slice 1: `cargo clippy --all-targets -- -D warnings` and ten focused storage tests passed (both mirrors, revision columns, indexed clock, historical journal seeding and five Linear upgrades). `check_migrations.py` accepted the expand migration.
- Installed lf created a Task, candidate edited its title, installed lf read and edited it again, and candidate read that edit. Both journals held 18 mutations.
- In-memory SQLite with 10,000 existing items: expand took 21.8 ms. 1,000 identifier lookups took 1.0 ms through `tracker_items_identifier`. Disk and lock wait are outside these timings.
- Historical upgrades seed at their original migration, through temporary views for the two journal tables. Upgrading an existing journal copies its rows once. Mirror comparisons use binary collation for case-only identifier edits.
- Slice 2: six config-key tests and four documented-command checks passed. A temporary Git repository exercised tracker status, journal, key creation and lookup, and the switch from Linear to Git. Installed lf read the Linear team written by the candidate. Offline Project edits passed in both connection modes, the server schema matched the candidate CLI, and clippy passed for all targets.
- Slice 3: 52 Rust DTO fixtures, the stream timestamp test, four documented-command checks and all-target clippy passed. Changed Swift sources parsed. A server restart replayed the creation receipt. Installed and candidate CLIs each replayed the other’s creation in an isolated Git repo; Task show and Work agreed on the unbound ID and sync state.
- Slice 4: 23 focused event, historical upgrade, schema and documented-command tests passed, plus the issue-fetch and end-to-end Task comment tests. All-target clippy passed. Candidate `lf help wave sync` names the tracker. The architecture HTML matches its source.
- Slice 5: all-target clippy, 52 Rust DTO fixtures, the server-restart replay test and four documented-command checks passed. A standalone Swift decoder read independent fetch and publication errors and rejected a missing fetch_error. In an isolated Git repository, both CLI versions replayed each other’s Task creation. The candidate reported fetch_error for its ref and printed “tracker writeback: current” after checkout.
- Slice 6: 53 Rust DTO fixtures, 57 Swift fixture tests and all-target clippy passed. The regenerated server schema matched byte for byte. The candidate explained a new Task in an isolated repository with a bound Task, a null sync time and a printed unknown.
- Compress after slice 6: one `fetch_from_tracker(repo, issue, refresh)` serves task run, moves and saved-Task lookup; `retained_task_input` fixture reads the `tracker` column (13 planning lookup tests had failed on `i.provider`). 21 planning lookup tests and clippy for execution and test-support passed.
- full suites — CI at gate
- Desktop: `scripts/test_desktop.sh --filter 'DTOFixtureTests|DesktopHeadlessTests|ServerProtocolTests'` passed after QA renamed `trackerAnswer` → `repoTrackerAnswer`

## Docs (written first)

Changed in place: `docs/work.md`, `docs/config.md`, `docs/surfaces.md`,
`docs/dev/planning.md`, `docs/dev/reference.md`, `docs/dev/data.md`,
`docs/dev/reference-models.md`, `docs/dev/reference-runtime.md`,
`docs/dev/reference-cutover.md`, `AGENTS.md`, `swift/README.md`,
`rust/loopflow/src/server/README.md`, `rust/loopflow-execution/src/ops/README.md`,
builtin `wave/skill/split-wave.md`.

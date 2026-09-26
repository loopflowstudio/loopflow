# Data model review: Session, Run, Work, Task, Wave

2026-09-26. Requested by Jack after two symptoms in this Task: the S6 design
proposed a `session-work.json` sidecar to bind a Session to a Task, and Jack
said "i dont love the sidecar record. can we just have a task field on Session
that can start null?" then "The fact that you didnt design in this way to
start makes me think we need some sort of data model review." Earlier, rename
took the same sidecar shape (`session-name.json`), and S5 records a
checkout-inferred subject as `Declared` because `AttributionSource` has no
other value.

Read-only. Line numbers are from this checkout at HEAD `ad82f11c9` plus the
uncommitted S1–S5 edits. **Observation** means the source or a Home on this
machine says so. **Proposal** means mine. Where the demo notes record a
decision as Jack's, I say which words were his.

## 0. The short version

- There is no Session record. "Session" is four read-time projections over
  four different storage owners (a Run directory, a `task_flow_positions`
  row, an `human-sessions/<id>.json` file, a `flows/<uuid>/position.json`
  file), assembled by `ops::human_session::list`
  (`rust/loopflow/src/ops/human_session.rs:588-598`).
- Every fact a human changes about a Session after launch (name, completion,
  which terminal holds it) is a file dropped beside the Run manifest. Work is
  the one such fact with no writer at all, so an interactive Session's Task is
  frozen at launch. That is the gap S5 and S6 are both working around.
- The "manifest is immutable" premise that produced the sidecars is not true
  today: `start_prepared` rewrites `manifest.json` in place at launch
  (`rust/loopflow/src/run_record.rs:1500-1543`).
- Work is stored as selector strings (`task:LOO-291`) and re-resolved through
  the full launch resolver on every read. That resolver requires an active PR
  (`rust/loopflow/src/ops/run.rs:172-176`), so a Session bound to a Task
  becomes an orphan in the sidebar the moment that Task's PR merges.
- The reason nobody designed a Session record: migration `0.11.036` deleted
  the old `task_sessions`/`project_sessions` tables under the doctrine
  "Project and Task are the durable product records; Run is their sole
  executor" (`rust/loopflow/src/store/migrations/0.11.036_delete_sessions.sql:1`).
  The human conversation was then built as a view of a Run, and each later
  need (name, completion, now Work) became a file beside the Run.
- Recommendation: make Session a store row with nullable `task_id`/`wave_id`,
  a `work_source`, and the name and its provenance as columns. Rename and
  bind become the same `UPDATE`. The Run directory stays immutable launch
  evidence. The draft-migration mechanism means this costs no ordinal race
  (`rust/loopflow/src/store/MIGRATIONS.md:10-20`), which removes the reason
  S6 gave for avoiding a table.

## 1. Inventory: what exists

### 1.1 Store rows (SQLite, `loopflow.db`)

| Table | Identity | Relevant columns | Source |
|---|---|---|---|
| `waves` | `WaveId` | name, repo, placement via `work_placements` | `store/migrations/0.10.001_initial.sql:1`; `0.11.036:114-130` |
| `projects` | `proj_…` | `wave_id`, `external_project_id`, `project_slug` | `0.11.031_durable_input_spine.sql:5` |
| `tasks` | `task_…` | `project_id`, `external_issue_id`, `issue_identifier`, `worktree` (UNIQUE), `workspace_slug`, `work_state`, `abandon_*` | `0.11.031:13-19`; `0.11.036:52-81`; `0.12.15:156-158`; read shape `store/sqlite/children.rs:1311-1316` |
| `task_prs` | `TaskPrId` | `task_id`, `sequence`, `branch` (UNIQUE), `merge_commit`, `abandoned_at`; one open PR per Task | `0.11.036:157-197` |
| `task_flow_positions` | `task_id` PK | `invocation_json`, `node_id`, `human`, **`session_run_id`**, **`ready_summary`**, `iteration`, `claim_json`, `failure_json` | `0.12.20.001_release.sql:9-25` |
| `task_events` | autoinc | `kind_json` (`started`, `progress`, `steer`, …) | `0.11.036:268-274`; `Started` written by `store/sqlite/chapters.rs:80-92` |
| `runs` | `run_…` | `epoch_id`, `home_id`, `state`, `source_kind`, `source_id`, `cwd` | `0.12.14.001_release.sql:88-110` |
| `work_placements` | one per Work | `home_id` | `0.11.036:114-130` |

Observations:

- `task_flow_positions.session_run_id` and `ready_summary` are Session facts
  living on the Flow position row. Rust reads them as `FlowPosition`
  (`rust/loopflow/src/durable.rs:131-142`) and `lf session ready` writes them
  (`human_session.rs:669-683`).
- The `runs` table has no reader or writer anywhere in `rust/loopflow/src`
  outside migrations (grep of `FROM|INTO|UPDATE|JOIN runs` over `*.rs`
  returns only a test name, `store/sqlite/durable.rs:1811`). Later
  migrations still declare foreign keys into it: `run_liveness.run_id`
  (`0.12.13.001_release.sql:42`), `project_events.run_id` (`:55`), a
  `supervising_run_id` (`:119`) and a `NOT NULL origin_run_id`
  (`0.12.8.001_release.sql:153`), so those columns can only ever hold NULL
  or migration-seeded ids, and the `NOT NULL` one implies its table is
  write-dead too. The store's
  Run row and the Run directory are two Run concepts; the row looks
  write-dead. Confirm the FK columns are unused before dropping either.
- Task identity is doubled on the wire: the roadmap's `PmTaskSummary.id`
  (Linear id, `lf/commands/waves.rs:137-145`) and
  `TaskRuntimeSnapshot.work_id` (`task_…`, `waves.rs:169-180`). Swift keys
  outline nodes by the first and joins Sessions by the second
  (`swift/LoopflowMac/WorkspaceProjection.swift:61-72`).
- `work_identities` (`store/sqlite.rs:500-535`) exports every Work with
  every selector alias (id, name/identifier, external id) so that selector
  strings on Run manifests can be matched back to rows (§1.4).

### 1.2 Files in a Run directory (`<home>/runs/<xx>/run_<uuid>/`)

| File | Written | Mutable after launch | Read by |
|---|---|---|---|
| `manifest.json` (`RunManifest`, `run_record.rs:195-217`) | `RunCapture::begin` (`:1804-1849`) | **Yes**: `start_prepared` rewrites harness, model, surface, cwd, repo, worktree, skill, subjects, `flow.or(...)`, host (`:1500-1543`) | every Session/Run reader (`:1221-1234`) |
| `manifest.subjects: Vec<SubjectAttribution{selector, source}>` | launch (`lf/commands/run.rs:771-776`, `human_session.rs:482-486`, `:508-513`, `flow_session.rs:133-141`) | no writer | `attributed_work` (`run_record.rs:2304-2347`), `WorkCatalog::resolve_run` (`lf/commands/work_catalog.rs:105-130`), `active.rs:225` |
| `manifest.flow: Option<RunFlowMembership>` | capture (`ops/flow_run.rs:215-230`) or prepare (`human_session.rs:487-489`) | `start_prepared` keeps the prepared value (`:1535`) | `run_flow_membership` (`human_session.rs:251-298`) |
| `context.json` | capture | no | evidence gap check |
| `events.jsonl` | recorder thread | append-only | usage, final answer, provider session fallback |
| `terminal.json` (`TerminalReceipt`) | `finish` | write-once (`:2187-2210`) | `RunSnapshot.outcome` |
| `prepared` / `launching` markers | prepare / start | renamed then removed (`:1495`, `:1509`, `:1541`) | `run_is_prepared` (`human_session.rs:1195-1204`) |
| `provider-session.json` (`ProviderSessionRef`) | provider callback | overwrite | resume, liveness |
| `provider-clients/<pid>.json` (`ProviderClientRef{pid, terminal_id, started_at}`) | client start (`:888-913`) | add/remove; `terminal_id` comes from `LF_TERMINAL_ID` only when the TTY matches (`:935-951`) | `terminal_ids`, state (`lf/commands/util.rs:283-290`) |
| `provider-client-stops/<pid>.json` | move/complete | add/remove | stop reason |
| **`session-name.json`** (`SessionNameRecord{title, source}`) | `lf session rename` (`:1079-1132`) under `.session-name.lock` | overwrite; a generated suggestion never replaces a human name | `session_name` (`human_session.rs:1375-1384`) |
| **`session-resolution.json`** | `lf session complete` (`:1152-1169`) | write-once | filters interactive Sessions out of the list (`:537-590`) |

`AttributionSource` is `Declared | Inherited` (`run_record.rs:182-186`).
`Inherited` is written by exactly one path, `lf replay`
(`lf/commands/replay.rs:72`). Everything else, including the S5 checkout
inference, writes `Declared`.

### 1.3 Other files that hold Session facts

| Location | Record | Home resolver |
|---|---|---|
| `<home>/human-sessions/<ask_id>.json` | `AskSessionRecord{id, parent_run_id, work, work_selector, title, detail, prompt, cwd, model, session_run_id, ready_summary, status}` (`human_session.rs:305-322`) | `current_home_lf_home_dir()` (`:2102-2104`) |
| `<home>/flows/<uuid>/position.json` | `FlowRun{id, flow, cwd, steps, cursor, wave, task, as_work, active: Boundary{run_id, run_dir, completed, ready_summary}}` (`ops/flow_run.rs:24-47`) | `current_home_lf_home_dir()` (`:62-67`) |
| `<home>/runs/…` | interactive Sessions | `observability_home_dir()` (`human_session.rs:1256`, `store/mod.rs:128-138`) |
| `loopflow.db` | Task Flow Sessions | `database_path_from_env` (`store/mod.rs:207`) |

Two Home resolvers and four locations for one concept. The memories on this
machine already record Home-divergence bugs in this class.

### 1.4 Projections computed at read time

`ops::human_session::list` (`:588-598`) concatenates four surfaces:

| Kind | Source of truth | Surface fn | Session `id` | `run_id` |
|---|---|---|---|---|
| Task Flow step | `task_flow_positions` + Run dir | `flow_surface` (`:1689-1754`) | `task:invocation:flow:node:iteration` (`:1990-2001`) | `position.session_run_id` (`:1700`) |
| Ask | ask json + Run dir | `ask_surface` (`:1756-1791`) | `ask_<uuid>` (`:437`) | `record.session_run_id` |
| Standalone Flow | `flows/<id>/position.json` + Run dir | `flow_session::session_surface` (`ops/flow_session.rs:169-204`), then `attribute_standalone_session` (`human_session.rs:1321-1338`) | `flow:<invocation>:<boundary>` (`flow_session.rs:13-15`) | `boundary.run_id` |
| Interactive | Run dir only | `interactive_surface` (`:1269-1319`) | **= `run_id`** (`:1290-1291`) | manifest |

`find_session` (`:602-639`) tries each store in turn to resolve an id.

Work resolution is its own chain, run once per Session per list:
`preferred_work_selector` picks the highest-ranked selector string
(`run_record.rs:2349-2367`) → `attributed_work` (`:2304-2347`) tries an exact
`task_…` parse, else calls `resolve_work_binding` → `resolve_work_selection`
(`ops/run.rs:101-236`), which loads the Task, Wave, Project, requires an
**active PR** (`:172-176`), renders the full Task prompt context
(`:177`) and loads the worktree config (`:206-210`). `lf runs --task` does not
use this chain; it uses `WorkCatalog::resolve_run` (`work_catalog.rs:105-130`),
a second matcher over the same selector strings with different rules
(ancestry disambiguation, no PR requirement). The active-Run reader is a third
caller (`run_record/active.rs:225`).

Swift then re-derives on top of the wire:

| Swift derivation | Where | What it recomputes |
|---|---|---|
| Session → Task attachment | `WorkspaceProjection.swift:47-72` | joins `session.work == .task(id: runtime.workId)`; a Task without a runtime snapshot can never own a Session |
| "orphan" | `:75`, `:272-280` | any Session not matched to a **visible** roadmap node. This is not `work == nil`; a Session whose Wave is not in this repo's roadmap is also an orphan here |
| breadcrumb / subject | `:79-117` | scans all Waves and Tasks per call |
| live / elsewhere / pending | `swift/LoopflowMac/Views/SessionsView.swift:134-155`, `:208-212` | from `terminalIds` ∩ local surfaces, and from whether a `move_here` action exists (`:141`, `:194`) |
| worktree grouping | `SessionsView.swift:316`, `:455` | by `cwd` string |
| launch scope | `swift/LoopflowMac/ConversationLaunch.swift:32-38`, `:43-57` | builds `--task <identifier>` / `--wave <name>` from the selected node |

### 1.5 Every Session field as the app shows it

| Field | Authored | Stored | Who can change it after launch | Readers that reconstruct it |
|---|---|---|---|---|
| `id` | per kind (§1.4) | not stored for interactive (it is the Run dir name) | Flow: changes on every loop visit because `cursor.iteration` is in it (`human_session.rs:1990-2001`); others never | 4 surfaces + `find_session` |
| `run_id` | capture | Run dir name; `task_flow_positions.session_run_id`; ask json; `Boundary.run_id` | Flow/Ask: replaced when the prepared Run is gone (`human_session.rs:1128-1131`, `:1155-1158`); interactive: never | same |
| `title` + `title_source` | seed: skill name or `word_pair(run_id)` (`:1280-1286`, `engine/naming.rs:26-34`); step name (`:1703-1710`); question title (`:445`, `:2088-2100`) | `session-name.json` beside the **Run**, else recomputed | `lf session rename` (`:1401-1447`); copied to the replacement Run by `carry_session_name` (`:1170-1193`) | `session_name` from all 4 surfaces; Swift renders `unavailable` for remote Flow Sessions (`:1706-1710`) |
| `work` | manifest `subjects` (interactive, standalone); `position.task_id` (Flow, `:1738`); `record.work` resolved at ask time from the parent manifest (`:434-441`) | manifest bytes / row PK / ask json | **nobody** for interactive and standalone; there is no writer of `subjects` after `start_prepared` | `attributed_work` chain (§1.4), `WorkCatalog`, `active.rs`; Swift join (§1.4) |
| `wave_id`, `work_path` | derived from `work` through the store (`:1340-1373`) | not stored | n/a | read-time, per Session |
| `flow_membership` | capture: `capture_membership` (`flow_run.rs:215-230`) or `RunFlowStep::of(position)` at prepare (`:487-489`) | `manifest.flow` (`Option`; `None` for manifests older than the field) | `start_prepared` keeps the prepared value (`run_record.rs:1535`) | `run_flow_membership` (`:251-298`), `of_position` (`:229-234`), `of_step`/`of_flow` (`flow_session.rs:191-194`) |
| `occurrence` | computed: captured `boundary_key` vs the store's current position (`run_record.rs:95-116`) | no | n/a | per read |
| `node`, `iterations` | `flow_iterations` at capture (`run_record.rs:59-72`) | `manifest.flow.step` as `Option` because older captures lack them (`:52-56`) | never | Swift `decodeIfPresent` (`swift/Loopflow/Models/SessionRecord.swift:50-58`) |
| `provider` | manifest `harness` | manifest | `start_prepared` (`run_record.rs:1523`) | `recorded_provider` (`:1392-1395`) plus two inline reads (`:1305`, `:1783`) |
| `ready_summary` | `lf session ready` | `task_flow_positions.ready_summary` / ask json / `Boundary.ready_summary`; interactive: always `None` (`:1308`) | `mark_ready` (`:659-700`, `flow_session.rs:206-220`); cleared on reopen (`:1131`, `:1158`) | 3 surfaces |
| `state` | derived: live provider clients + `provider-session.json` + `session-resolution.json` (`:1586-1612`, `util.rs:283-290`) | files | client start/stop/complete | Rust once; Swift again (§1.4) |
| `actions` | `session_actions(kind, state)` (`:93-144`) | no | n/a | Swift reads `move_here` presence as a state bit |
| `terminal_ids` | `LF_TERMINAL_ID` receipt (`run_record.rs:935-951`) | `provider-clients/<pid>.json` | client lifecycle | Swift `localTerminal(for:)` |
| `cwd` | manifest / `task.worktree` / ask json / `run.cwd` | manifest | `start_prepared` | Swift worktree grouping |
| `detail` | `harness:model`, step name, skill | manifest | never | — |

## 2. Mismatches

### 2.1 Session attributes stored as launch-time Run bytes

`work`, `title` seed, `provider`, `cwd`, `flow_membership` are all read from
`manifest.json`. Of these, only `provider`, `cwd` and `flow_membership` are
facts about the launch. `work` and the name are facts about the conversation.
The design already treats them that way: rename exists, bind is wanted, and
`carry_session_name` (`human_session.rs:1170-1193`) copies the name from one
Run to the next precisely because the name belongs to the Session and the
storage belongs to the Run.

### 2.2 Post-launch mutations bolted on as sidecars

Three today: `session-name.json` (name), `session-resolution.json`
(completion), `provider-clients/` (attachment). S6 would add a fourth. Each
has its own lock, schema version, staging-rename dance and reader. The
premise behind them, "the manifest is immutable launch evidence", is already
false: `start_prepared` (`run_record.rs:1500-1543`) rewrites the manifest at
launch, and `validate_manifest_path` (`:1185-1204`) checks only that the
directory name matches `run_id`; nothing fingerprints the content. The S6
note's citation of "path validation" as the immutability guard points at this
function.

### 2.3 Work has no writer and is re-resolved through the launch resolver

Observations, in order of consequence:

1. `manifest.subjects` is written at capture (`run_record.rs:1839`) and
   rewritten by `start_prepared` from the same launch inputs (`:1529`). No
   `lf` verb, op or store call changes it afterwards: a grep for
   `manifest.subjects =` / `subjects: spec` over `rust/loopflow/src` finds
   those two lines, the pre-capture `built.subjects` assignments in
   `lf/commands/run.rs:35,60,83`, and tests.
2. Selectors are strings in two dialects for the same concept:
   `task:LOO-291` from `resolve_work_selection` (`ops/run.rs:188-192`, also
   the S5 path and every `--task` launch from the app) and `task:task_<hex>`
   from Flow preparation (`human_session.rs:482-486`). `attributed_work`
   short-circuits only the second (`run_record.rs:2310-2321`).
3. For the identifier dialect, every read of every interactive Session
   calls `resolve_work_selection`, which requires an active PR
   (`ops/run.rs:172-176`) and renders the full Task prompt context
   (`:177`). When the Task's PR merges and no successor PR exists, the call
   errors, `attributed_work` logs a warning and returns `None`
   (`run_record.rs:2338-2345`), and the Session lists as `work: null`.
   **A bound Session becomes an orphan when its PR lands.** The S5 note
   observes the same PR rule at launch time and treats it as a launch
   question; it is also a read question, and it applies retroactively to
   every Session already bound.
4. `lf runs --task` does not share this reader. `WorkCatalog::resolve_run`
   (`work_catalog.rs:105-130`) matches selector aliases from
   `work_identities` with no PR rule. So after a merge, `lf runs --task LOO-291`
   still lists the Run while `lf session list` shows it unbound. Two readers,
   two answers, the forbidden outcome named in the S6 design.

### 2.4 Identity: Session id = Run id, and what it constrains

- Interactive: `id == run_id` (`human_session.rs:1290-1291`). A Session can
  never outlive or replace its Run; a "conversation" that resumes under a new
  provider session is a different Session. This is why completion is a file
  in the Run dir and why the name lives there.
- Task Flow: the id embeds `cursor.iteration` (`:1990-2001`), so the same
  human boundary revisited on the next loop is a new Session id; and the
  `run_id` behind an id is replaced when the prepared Run is lost
  (`:1128-1131`). Here Session ≠ Run already, which is why `carry_session_name`
  exists.
- Ask: own uuid, own file, own `session_run_id` that can be replaced
  (`:1155-1158`).

So the code already has 1 Session : N Runs for two of four kinds, and the
only kind with 1:1 identity is the one the sidebar mostly shows. The
constraint this imposes on S6: "bind" cannot be expressed as "attach this
conversation to that Task" because the conversation has no record to attach;
it has to be expressed as "edit this Run's evidence".

### 2.5 The Declared/Inherited gap

`AttributionSource::Declared | Inherited` (`run_record.rs:182-186`) was made
for one distinction: typed on the command line versus inherited by `lf
replay` (`replay.rs:72`). It cannot say "resolved from the checkout branch"
(S5) or "assigned by a human after launch" (S6). S5 therefore records
inference as `Declared`. Because the source rides on the manifest, adding a
variant is a wire change to launch evidence; on a Session record it would be
an ordinary column.

### 2.6 Swift re-deriving what Rust knows

- Orphan ≠ `work == nil`. Swift's `unmatchedSessions`
  (`WorkspaceProjection.swift:75`) is "not matched to a visible roadmap node",
  which also catches Sessions bound to a Task the roadmap does not list
  (another repo's Wave, a Task without a runtime snapshot, a chapter gap).
  The sidebar header "Sessions launched without a Task or Wave"
  (`WorkspaceNavigator.swift:334`) is therefore sometimes wrong.
- Two Task ids per row (`:61-66`). The join works only because
  `TaskRuntimeSnapshot.work_id` is present; an unstarted Task with
  `runtime == nil` cannot own a Session even if one is bound to it.
- Liveness (`SessionsView.swift:134-155`): Rust already computes `state` and
  `actions`; Swift recomputes a three-way state from `terminalIds` and the
  presence of `move_here`. This is presentation of local surfaces, so some
  of it is legitimate, but reading an action's existence as a state bit is a
  second vocabulary.

### 2.7 DTO fields that are Optional for storage reasons, not product reasons

| Field | Why it is Optional today | Product meaning |
|---|---|---|
| `provider` | manifest may be unreadable or on another Home (`human_session.rs:1727-1731`) | a Session always has a provider |
| `flow_membership.unknown{reason}` | manifests older than the `flow` field (`run_record.rs:120-125`) | a conversation is either in a Flow step or not |
| `node`, `iterations` | older captures (`:52-56`); Swift `decodeIfPresent` (`SessionRecord.swift:50-58`) | always known for a step |
| `title_source: unavailable` | the name file lives on another Home (`:1034-1039`, `:1706-1710`) | a name is generated or human |
| `wave_id`, `work_path` | derived duplicates of `work` | one Work reference |
| `ready_summary` on interactive | never set (`:1308`) | kind-dependent; fine |

Counts on this machine (read-only scan of `manifest.json` files):

| Home | Run dirs | `flow` absent | `session-name.json` | `session-resolution.json` |
|---|---|---|---|---|
| `~/.lf` (production) | 340 | 339 | 0 | not counted |
| `~/.lf-dev/installed/local-afee63d7…` (demo) | 174 | 152 | 1 | 11 |

So "Flow membership unknown" is the common case, not an edge, and the
rename sidecar has been used once. Any migration of sidecars is trivial.

### 2.8 Why it ended up this way

`0.11.036_delete_sessions.sql:1`: "Project and Task are the durable product
records; Run is their sole executor." That "Session" was the Task worker
process. The human conversation feature that followed honored the doctrine by
making Session a view over Runs and putting nothing new in the store. Each
later human-facing need (name, completion, attachment, now binding) then had
to live beside the Run. Jack's question "why didn't you design it this way to
start" has that answer: the doctrine forbade a Session row, and the manifest
was declared immutable, so sidecars were the only remaining shape. Both
premises are worth revisiting rather than adding a fourth sidecar.

## 3. Proposed model

### 3.1 Concepts (1:1 with the real world)

| Concept | Real thing | Record | Mutable? |
|---|---|---|---|
| Wave | durable responsibility | `waves` row | yes |
| Task | one planned piece of work with a worktree and PRs | `tasks` row | yes |
| Run | one provider launch: prompt, harness, model, cwd, events, outcome | Run directory | **no** after publish (drop the `start_prepared` rewrite; see 3.4) |
| **Session** | one human conversation: openable, nameable, bindable, completable; may span Runs | **`sessions` row** (new) | yes |
| Flow position | where a Task's managed Flow is | `task_flow_positions` row | yes |

### 3.2 The `sessions` row

Proposal (column names illustrative):

```sql
CREATE TABLE sessions (
    id            TEXT PRIMARY KEY,                 -- "sess_…" newtype
    kind          TEXT NOT NULL CHECK (kind IN ('interactive','ask','flow','standalone_flow')),
    run_id        TEXT NOT NULL,                    -- current Run; history is Runs whose manifest names this session
    task_id       TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    wave_id       TEXT REFERENCES waves(id) ON DELETE SET NULL,
    work_source   TEXT CHECK (work_source IN ('declared','checkout','inherited','bound')),
    title         TEXT NOT NULL,
    title_source  TEXT NOT NULL CHECK (title_source IN ('generated','human')),
    cwd           TEXT NOT NULL,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    resolved_at   INTEGER,                          -- completion
    CHECK ((task_id IS NULL) OR (wave_id IS NOT NULL)),
    CHECK ((task_id IS NULL AND wave_id IS NULL) = (work_source IS NULL))
);
```

- `task_id` and `wave_id` start null. This is Jack's sentence made literal:
  "a task field on Session that can start null."
- `work_source` answers S6's Q8 without a second record and without touching
  `AttributionSource` on the manifest.
- Rename and bind are the same operation: `store.update_session(id, patch)`.
  The human-beats-generated rule for names becomes a `WHERE title_source !=
  'human' OR ?source = 'human'` clause instead of a file lock.
- Completion is `resolved_at`, replacing `session-resolution.json`.
- `ready_summary` stays where the boundary is (`task_flow_positions`, ask
  record) in phase 1; phase 3 can move it here if Flow/Ask rows join the
  table.

### 3.3 What Run keeps

The manifest keeps exactly what was true at launch: harness, model, surface,
cwd, repo, worktree, skill, `subjects` (as launch attribution evidence),
`flow` capture, context ref, runtime identity. It gains one field:
`session_id: Option<SessionId>` written at capture (the Session row is
created before or with the Run). Old manifests decode `session_id: None`,
which is correct: those Runs predate Sessions as records.

`subjects` stop being the Session's Work. They remain "what the launch
declared", which is what `lf usage`, `lf activity` and cost attribution
consume. `AttributionSource` can then stay `Declared | Inherited`; the
checkout inference is a Session fact (`work_source = 'checkout'`), not a
manifest fact. If Jack wants the manifest to also say the launch was
checkout-inferred, add the variant; it is not required by this model.

### 3.4 One reader for Work

`fn run_work(store, run) -> Option<WorkRef>`:

1. If the Run's manifest carries `session_id` and that row exists, the Work
   is the row's `task_id`/`wave_id`.
2. Else the manifest's preferred subject, resolved by **exact id or
   `work_identities` alias only** (the `WorkCatalog` rules), never through
   `resolve_work_selection`.

Callers: `interactive_surface`, `attribute_standalone_session`, `ask_surface`
(for its parent), `WorkCatalog::matches_run`, `active.rs`. This deletes the
`attributed_work` → `resolve_work_binding` → `resolve_work_selection` path
from reads, which removes the active-PR requirement and the prompt render
from `lf session list`. The launch resolver stays a launch resolver.

### 3.5 What gets deleted

| Delete | Replaced by |
|---|---|
| `session-name.json`, `SessionNameRecord`, `read/write_session_name`, `.session-name.lock` (`run_record.rs:1034-1132`) | `sessions.title`, `title_source` |
| `carry_session_name` (`human_session.rs:1170-1193`) | the row outlives the Run; `run_id` is updated on replacement |
| `session-resolution.json`, `provider_session_is_resolved`, `resolve_provider_session` (`run_record.rs:1134-1169`) | `sessions.resolved_at` |
| `attributed_work` (`run_record.rs:2304-2347`) and the `"project"` historical branch | `run_work` (3.4) |
| the `start_prepared` manifest rewrite (`run_record.rs:1500-1543`) | prepare creates the Session row and a Run with its final launch inputs; or prepare writes no manifest until launch |
| `SessionTitleSource::Unavailable` (`run_record.rs:1038`) and the Swift case | remote Sessions read the name from their Home's row through the existing remote route, or the wire says `title: null` |
| Swift `unmatchedSessions` as the orphan predicate (`WorkspaceProjection.swift:75`) | orphan ⇔ `work == nil` on the wire; a Session bound to Work outside the visible roadmap gets its own row detail, not the orphan bucket |
| the store `runs` table and its FK columns, if 1.1's observation holds | Run directories |

Not deleted: `provider-clients/`, `provider-session.json`, `terminal.json`,
`events.jsonl`, `context.json`. Those are Run-lifetime evidence.

### 3.6 Identity

- Interactive Sessions get a `sess_…` id distinct from `run_id`. The wire's
  `id` changes for interactive Sessions. Swift keys panes and surfaces by
  Session id (`SessionsView.swift:208-212`, `:456-458`), so this is a rename
  of a string on the same code path, not a re-architecture. An `lf session
  open run_…` argument resolves through `sessions.run_id` as `find_session`
  already resolves boundary Runs (`human_session.rs:1459-1495`).
- Task Flow Session ids keep their tuple form in phase 1; a `sessions` row
  with `kind='flow'` can take over the id in phase 3 so a boundary revisited
  on the next loop is the same conversation with a new Run.

### 3.7 The alternative Jack's note leans toward: manifest as the mutable record

The demo notes record "Session.work is the nullable field … its backing store
is the Run manifest's subjects, updated in place" as Jack's decision. Jack's
verbatim words were only "can we just have a task field on Session that can
start null?"; the backing-store choice was the agent's extrapolation. Costs
and benefits of that option, honestly:

| | Manifest as record | `sessions` row |
|---|---|---|
| Change size | small: a manifest lock, `subjects` rewrite in `lf session bind`, fold `session-name.json` into a `session` object | medium: draft migration, `Store` API, four surfaces read the row, Swift id rename |
| Readers of Work | unchanged (`attributed_work` + `WorkCatalog`), still two, still PR-gated unless 3.4 is done anyway | one (3.4) |
| Session ≠ Run | entrenched for interactive; Flow/Ask keep separate records | one record for every kind, phased |
| Run evidence | mutable; `lf usage`/`activity` attribution moves on bind (S6's done-when wants `runs --task` to move, so this is arguably desired) | immutable; history reads through the row |
| Name carry on Run replacement | still needed | gone |
| Old manifests | decode with `subjects` as before | decode with `session_id: None` |
| Home divergence | Run dir and store may be different Homes today (§1.3); binding writes to the Run's Home | binding writes to the store; the Run dir is only read |

I recommend the row. The manifest option is the smaller diff, but it keeps
Session as a view and leaves the read path that turns bound Sessions into
orphans after a merge. The row is the shape that makes S6's bind, S5's
provenance, rename, completion and the orphan predicate the same kind of
thing.

## 4. Migration and risk

### 4.1 What existing Homes hold

- Production `~/.lf`: 340 Run dirs, no `session-name.json`, 339 manifests
  without `flow`. Nothing to migrate but a schema draft.
- Demo Home: 174 Run dirs, one `session-name.json`, 11
  `session-resolution.json`, 2 Ask records, 1 standalone Flow record.
- Neither Home has any Session row to lose. Backfill is optional: create
  `sessions` rows lazily on first list for interactive Runs that are
  unresolved (`scan_unresolved_provider_runs` already knows which), reading
  the one name sidecar if present, then ignore sidecars forever. Or backfill
  in the migration; there is nothing at scale.

### 4.2 Older installed `lf` in a shared Home

The repo rejects compatibility shims (CLAUDE.md, "Don't maintain backwards
compatibility"). Consequences, stated rather than mitigated:

- An older `lf` reading a manifest with a new `session_id` key ignores it:
  `RunManifest` derives plain `Deserialize` with no `deny_unknown_fields`
  (`run_record.rs:194-195`), so serde drops the key. It keeps showing the
  launch subjects. Two `lf` binaries on one Home disagree about a bound Session
  until the old one is gone. This is the same class of divergence the
  cycle-06 review recorded for `step_index`/`iteration`.
- An older `lf` opening a store with the `sessions` table refuses at the
  migration frontier, which is today's behavior for every migration
  (memory: installed lf lags the store). Not new.
- The demo Home's single `session-name.json` is read once by the new binary
  and never written again; the old binary would still write it. Accept.

### 4.3 Smallest PR sequence

1. **Read path first, no schema.** Replace `attributed_work`'s resolver call
   with the `WorkCatalog` alias match (3.4 step 2). Removes the PR gate and
   the prompt render from every Session read; makes `session list` and `runs
   --task` agree. One Rust test: a Session bound to a Task whose PR merged
   still lists the Task. This is a bug fix independent of the model.
2. **`sessions` draft migration + Store API + interactive Sessions.** Row
   created at interactive capture; `interactive_surface` reads title,
   provenance, Work, resolution from the row; `lf session rename` and
   `complete` become row updates; delete the two sidecars and
   `SessionTitleSource::Unavailable`'s write path. Wire: `id` becomes
   `sess_…`; DTO fixtures move. Swift: none beyond the id string.
3. **S5 on the row**: checkout inference writes `task_id` +
   `work_source='checkout'`. **S6 bind**: `lf session bind` is
   `update_session`. `Bind` action legality from the row's kind.
4. **Ask and Task Flow Sessions as rows** (optional, later): `kind='ask'`
   replaces `human-sessions/*.json`; `task_flow_positions.session_run_id` →
   `session_id`; `carry_session_name` and `ready_summary` duplication go.
5. Delete the store `runs` table after confirming 1.1.

Each PR is independently landable; 1 is worth landing regardless of the
answer to §6.

## 5. What this changes for S5 and S6

### S5 (checkout binding), on the proposed model

- `checkout_binding` (`lf/commands/run.rs:93-109`) still resolves the Task
  through `task_for_checkout` (`ops/task.rs:1064-1076`), but the result is
  written as the Session row's `task_id` with `work_source = 'checkout'`, and
  the manifest's `subjects` stay whatever the launch declared. Whether to
  also list the Task in `subjects` is a choice about launch evidence, not
  about the Session.
- The active-PR requirement in `resolve_checkout_binding` (`ops/run.rs:244-250`)
  was added to avoid an error thrown by the launch resolver, not as product
  intent. With Work resolved from the row, binding to a Task whose PR has
  merged is a plain nullable FK and needs no rule; the question of whether
  it *should* bind is §6 Q3.
- `Started` recording (`run.rs:818-838`) is unchanged: it keys off the
  launch's Task, which is now the row's `task_id`.
- The S5 proof (`lf_launches_inside_a_task_checkout_bind_to_that_task`)
  asserts `work` on `session list` and `runs --task`; both hold through
  `run_work`. The "landed branch launches unbound" step becomes a decision,
  not a workaround.

### S6 (bind and the control room), on the proposed model

- `lf session bind <id> --task LOO-291 | --wave product | --repository`
  becomes `update_session(id, { task_id, wave_id, work_source: 'bound' })`.
  No sidecar, no manifest edit, no second selector reader. `--repository`
  sets both to null and `work_source` to null.
- Legality: `Bind` is available for `kind = 'interactive'` (and
  `standalone_flow` if wanted); for `flow` and `ask` the row's Work is its
  execution, so the action carries `unavailable_reason` as the design says.
  This is a kind check on the row, not a file probe.
- Done-when from the S6 note holds by construction: `title`,
  `flow_membership` and the manifest are untouched by bind; `runs --task`
  follows the row through `run_work`.
- Orphan predicate: the sidebar's section becomes `work == nil` on the wire.
  Swift stops computing `unmatchedSessions` from roadmap visibility
  (`WorkspaceProjection.swift:75`); a Session bound outside the visible
  roadmap shows its `work_path` and is not an orphan.
- The control room and the picker are unchanged by the model; the picker's
  "matches this checkout" hint is still presentation of `cwd` vs
  `workspace.worktree`.

## 6. Questions only Jack can answer

1. **Row or manifest.** The demo notes recorded "backing store is the
   manifest's subjects" as your decision; your words were only "a task field
   on Session that can start null". Do you want the `sessions` row (my
   recommendation, §3.7) or the manifest-as-record cut?
2. **Session identity on the wire.** Interactive Session `id` stops equalling
   `run_id`. Acceptable now (phase 2), or keep `id == run_id` until Flow/Ask
   rows land?
3. **Binding to a Task whose PR has merged, or whose `work_state` is done.**
   With a nullable FK nothing prevents it. Allow, allow but show the Task as
   completed, or refuse in `bind` only?
4. **Should Run evidence follow the Session?** After a bind, should `lf
   usage`/`lf activity` attribute the Run to the new Task (read through the
   row) or keep the launch attribution? §3.4 makes them follow; the manifest
   stays as it was.
5. **Ask and Task Flow Sessions as rows (phase 4)** or leave those two on
   their current records indefinitely? This decides whether
   `carry_session_name` and the `ready_summary` duplication ever go away.
6. **The store `runs` table.** No Rust code reads or writes it, but three
   later tables carry foreign keys into it (§1.1). Delete it and those
   columns in this pass, or leave it for a separate cleanup?

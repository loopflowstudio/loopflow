# Data model: the user's objects, their APIs, and what the infrastructure maps

2026-09-26. Product-first review requested by Jack: derive the objects and the
relationships from the product, write the APIs those objects need from each
other, then check whether the infrastructure maps that shape or introduces hops.
Read-only; no source changed. `[obs]` is an observation with a citation;
`[proposal]` is mine; `[Jack]` is a decision recorded in
[demo-native-workspace.md](demo-native-workspace.md). Line numbers are this
checkout at the time of writing. The parallel infrastructure inventory
(`scratch/data-model-review.md`) did not exist when part 3 was written, so
part 3 cites source directly.

## Part 1 — The objects as Jack uses them

Derived from [main-view-task.md](main-view-task.md), the S6 note, the
prototypes (`visual-study/mockups.html`, `task3-study.html`,
`polish/index.html?v=d`) and the S1–S4 native captures. Where the prototype
gives a Session a field, that is what the user's Session is made of
(`visual-study/polish/polish.js:107-132`: `task` (nullable), `name`,
`provider`, `state`, `member` (Flow node + label, nullable), `cwd`, `summary`).

| Object | What Jack does with it | Made of, in his mind | Lifecycle |
|---|---|---|---|
| **Repository** | Scopes the whole window; switches between repos in the sidebar head | Name, a set of Waves, a set of worktrees, a set of orphan Sessions | Registered once; lives as long as the checkout exists |
| **Wave** | Clicks it for objective → current KRs → Task plan; starts Wave-scope conversations | Name, objective, KRs, its Tasks, a recommended Flow | Durable; its *chapter* rotates the plan underneath it (chapter/Project is internal, never navigated) |
| **KR** | Reads whether it holds and over what window | Text, holds/not, a window | Replaced with the chapter |
| **Task** | Selects it in the sidebar (only started Tasks are there), reads the title/Description/Comments, starts or restarts its Flow, opens a new Session on it, looks at recent Runs | Issue ID and title (from Linear), a Wave, a worktree and branch, a PR, a Flow with a position, zero or more Sessions, a Run history, comments, "started" | Filed in Linear → prepared (worktree + branch) → started (first Run or Flow) → PR open → merged/done; unstarted Tasks live only on the Wave page |
| **Flow** | Picks one before start (typeahead), sees the pinned diagram after, Stop & restart… to swap it | A named path of steps with two return loops and an iteration tuple | Recommended → pinned to a Task (an *invocation*) → finished or replaced |
| **Step / occurrence** | Reads which one is current, which are done this pass, clicks a Session's membership chip to land on it | A lowercase skill name at an exact position in one invocation | Current → completed for this pass → repeated on the next return |
| **Run** | Discloses "Recent runs" under a Task; sees the running status line `● review-slice · 12m · claude`; Jack: "Runs should have tasks too" | Provider, skill, start/end, outcome, *the Task it was for* (nullable), the Flow step it executed (nullable) | Launched → running → ended; history kept |
| **Session** | The unit he drills into (Wave → Task → Session), renames, binds to a Task, completes; the terminal *is* the Session | A name, a provider, a state (working / your turn / closed), at most one Task (nullable, Jack: "a task field on Session that can start null"), a Flow membership (step / independent / unknown), a cwd, a summary, the terminal with its scrollback and draft | Launched from a Task title, a Wave, ⌘K, `lf` in a terminal, or a companion pane → active → ready (for review Sessions) → completed; orphans get bound later |
| **Session name** | Sees the generated word pair, renames in place, expects a human name to stick | A title plus who chose it | Generated at launch → optionally human; survives Run replacement |
| **Worktree / branch** | Sees it as a quiet mono chip; opens companion shells in it | A directory on disk tied to a Task's PR branch | Created by prepare; outlives the PR |
| **PR** | Sees `PR #899 open` on the delivery tail; the issue ID links to Linear, the PR to GitHub | Number, branch, state | Started → published → merged/abandoned; a Task may chain several |
| **Comment** | Expands "Comments (n)" under Description | Author, date, Markdown body | Written in Linear; read-only here |
| **Pane / companion shell** | Splits beside a Session; expects it to keep running when he navigates away | A terminal in a worktree; not a Session | Retained with the window |

Two facts Jack keeps separate and the design insists on: a Session's **Task**
(ancestry, nullable, changeable) and its **Flow membership** (which exact
occurrence it executes, never inferred) —
[session-launch-surfaces.md](session-launch-surfaces.md) "Two axes, one rule".

## Part 2 — Relationships and the APIs the objects need from each other

```
Repository 1 ──< Wave 1 ──< Task 1 ──< PR (one active)
                     │           │
                     │           ├──< Run        (Run.task nullable; Run.wave nullable)
                     │           │      └── Session  (a Run that has a conversation; 0..1 per Run)
                     │           └── Flow invocation ──< step occurrence
                     │                       ▲
                     │                       └── Session.membership (nullable, never inferred)
                     └──< Session (wave-scope: Run.task = null, Run.wave = W)
Repository ──< orphan Session (Run.task = null, Run.wave = null)
```

Cardinalities Jack stated or accepted: a Session belongs to **at most one**
Task and can start with none [Jack]; a Task has **zero or more** Sessions and
a Run history; a Run belongs to at most one Task [Jack: "Run.task =
Session.task"]; a Session is one Run's conversation (its id *is* the Run id in
the design's Session section, "SessionRecord already has id, runId"); a Flow
occurrence has zero or more Sessions; a Task has one active PR and one
worktree; a Session's Flow membership is orthogonal to its Task.

### Session
| API | Needed by | Nullable / notes |
|---|---|---|
| `session.task` → Task or none | sidebar grouping, breadcrumb, control room | starts null; changed by bind |
| `session.wave` → Wave or none | Wave-scope conversations | null for orphans |
| `session.membership` → {flow, occurrence, tuple, current/earlier/past} or independent or unknown | Session rows, breadcrumb chip → graph node | never derived from task/cwd/provider |
| `session.name`, `session.name_source` | rows, breadcrumb, rename | human beats generated |
| `session.provider`, `state`, `summary`, `cwd` | rows, status | provider is the Run's harness |
| `bind(task)` / `bind(wave)` / `unbind()` | control room, ⌘K, Task-page rows [Jack: universal bind] | must not touch name, id, membership, panes |
| `rename(title)` | breadcrumb pencil, agent suggestion | reversible |
| `open()`, `complete()` | drill-down, toolbar | legality from Rust |

### Task
| API | Needed by |
|---|---|
| `task.sessions` → the Sessions whose `task` is this Task | sidebar count, drill-into-one, Task page list |
| `task.runs` → Runs whose `task` is this Task, newest first | Recent runs disclosure, status line elapsed |
| `task.flow` → recommended, or pinned {graph, current, completed, returns, tuple, execution, controls} | Flow panel |
| `start(flow)`, `resume()`, `restart(flow)` | Flow controls |
| `new_session()` → launches a Session with `task` set, in the Task's worktree, before or during a Flow | New session button |
| `task.started` | sidebar membership |
| `task.worktree`, `task.active_pr`, `task.wave` | chips, breadcrumb, delivery tail |
| `task.comments` | Comments (n) |
| `task.changed_since(when)` → events, comments, Runs since Jack last looked | "what changed since I looked" (asked for, not built) |

### Run
| API | Needed by |
|---|---|
| `run.task` / `run.wave` (nullable) and how they were set (declared / checkout / bound / inherited) | Recent runs, Session grouping, evidence |
| `run.step` → exact Flow occurrence or independent or unknown | Session membership, Task status line |
| `run.session` → the conversation on this Run, if any | drill-down |
| `run.provider`, `skill`, `started`, `ended`, `outcome` | Recent runs rows |

### Flow / occurrence
| API | Needed by |
|---|---|
| `flow.graph`, `occurrence(node)` → current / completed-this-pass / return counts | diagram |
| `occurrence.sessions` → Sessions whose membership is this node | chip → node, node → Sessions |

### Wave, Repository
| API | Needed by |
|---|---|
| `wave.tasks` (with `started`), `wave.krs`, `wave.objective`, `wave.recommended_flow` | Wave page, sidebar |
| `wave.sessions` (task = null, wave = W) | Wave-scope conversations |
| `repo.waves`, `repo.orphan_sessions` (task = null, wave = null) | sidebar, control room |

## Part 3 — How the infrastructure serves each API

A *hop* is a representation change or an extra read the user's model does not
have: a typed id becoming a prefixed string and back, a file becoming a struct
that is then re-grouped, or the same fact read twice from two stores.

### The records that exist `[obs]`

| Record | Where | Holds |
|---|---|---|
| Run manifest | `~/.lf/…/runs/<2>/<run_id>/manifest.json`, layout `rust/loopflow/src/run_record.rs:592-617`, `:2108-2111` | `harness`, `skill`, `cwd`, `surface`, `subjects: Vec<SubjectAttribution>`, `flow: Option<RunFlowMembership>` (`run_record.rs:194-215`) |
| Subject | `run_record.rs:166-169` | `selector: String` like `"task:LOO-291"`, `source: Declared \| Inherited` (`:180-185`) |
| Session name | `session-name.json` beside the manifest, `run_record.rs:1056-1131` | `{title, source}`; copied to a replacement Run by `carry_session_name` (`ops/human_session.rs:1170-1193`) |
| Interactive Session | none — a predicate over Run dirs: `surface == "tui"` or a `provider-clients/` dir (`run_record.rs:1016-1018`) and an unresolved `provider-session.json` (`:537-580`) | id = run id (`human_session.rs:1284`) |
| Flow Session | a row of `task_flow_positions` (`store/migrations/0.12.20.001_release.sql:9-19`), `FlowPosition{session_run_id, ready_summary,…}` (`durable.rs:131-143`) | id = `task:invocation:flow:node:iteration` composite (`human_session.rs:1990-2001`) |
| Ask Session | `human-sessions/<ask_uuid>.json` (`human_session.rs:305-323`, `:2102-2104`) | own `work`, `work_selector`, `title`, `session_run_id` |
| Standalone Flow Session | `flows/<uuid>/position.json` (`ops/flow_session.rs:88-89`) | Run id on the boundary |
| Task | `tasks` (`0.11.031_durable_input_spine.sql:13-19`), `task_prs` with `branch UNIQUE` (`0.12.4.001_release.sql:110-124`), `task_events` (`0.11.036_delete_sessions.sql:268-274`) | no Run column anywhere |
| Store `runs` table | `0.12.14.001_release.sql:89-110`: `source_kind IN ('wave','project','task','migration')`, `source_id` | the Wave-runtime epoch Run; **a second thing called Run**, with the Task FK the manifest lacks; no non-migration writer found (`grep "INSERT INTO runs"` hits only `store/migrations.rs`) |

So `lf session list` (`human_session.rs:588-599`) concatenates four sources
(Flow rows, Ask files, standalone Flow dirs, interactive Run dirs) and dedups
boundary Runs (`:1210`, `:1262-1264`). Three id schemes, four stores, one list.

### API by API

| API (part 2) | Path today | Hops | Flags |
|---|---|---|---|
| **`session.task`** | manifest `subjects` → `preferred_work_selector` ranks by string prefix `task:`>`project:`>`wave:` (`run_record.rs:2349-2367`) → `split_once(':')` → `TaskId::parse`, else `resolve_work_binding(store, cwd, selector)` (`:2337`) → `resolve_work_selection` (`ops/run.rs:129-231`) which is the **launch** resolver: it loads Wave and Project and **requires an active PR** (`:172-176`) → `WorkRef` → `session_wave_id` re-reads the Task (`human_session.rs:1340-1350`) → `session_work_path` re-reads the Task and Wave to build a display string (`:1352-1373`) → wire `work`, `wave_id`, `work_path` → Swift `SessionRecord.work` (`SessionRecord.swift:138-140`) | 6, three store reads of the same Task | **F1** the Task is a `task:` string inside a ranked list, not a field. **F2** reading a Session's Task through the launch resolver: a Task whose PR has merged reads as *no Task* (warning at `run_record.rs:2340`). **F3** `wave_id` and `work_path` are derived display fields on the wire. **F4** the internal Project leaks as a `project:` subject (`ops/run.rs:187-191`) and a `WorkRef::Project` branch (`run_record.rs:2318-2336`) |
| **`task.sessions`** | no Rust API (`SessionCommand::List` has `--all`, no `--task`, `lf/mod.rs:726-732`) → Swift `WorkspaceProjection.init` filters all Sessions per Task by `$0.work == work` (`WorkspaceProjection.swift:45-50`, `:64-69`) → rebuilt on **every** `model.workspace` access (`PodiumModel.swift:88-90`), called from eight body sites (`SessionsView.swift:362,450,474,1079`, `WorkspaceNavigator.swift:29,173,290,295`, `WorkSurfaceView.swift:197`) | 2 + N re-groupings per frame | **F5** the join lives in Swift and is recomputed per body; `runs --task` exists but `session list --task` does not |
| **"which Task is this Session"** in the app | `subject(for:)` scans waves → tasks → sessions to find the Session, then returns the Task (`WorkspaceProjection.swift:107-115`), used by `openSession` (`SessionsView.swift:450`) and pane focus (`:1079`) | reverse join | **F6** the record already carries `work`; the app re-derives it from the grouping |
| **`session.membership`** | manifest `flow` (`RunFlowMembership::Step(RunFlowStep)`, `run_record.rs:44-56`, `:118-124`) → `run_flow_membership` checks the store's Flow position for current/earlier/past (`human_session.rs:251-303`) → `SessionFlowMembership::of_step` (one shape since the compress pass) → Swift enum with `decodeIfPresent` for `node`/`iterations` (`SessionRecord.swift:56-57`) → label → breadcrumb chip selects the node (`WorkspaceBreadcrumbBar.swift:148-172`) | 2 + one store read | Natural. **F7** `node`/`iterations` are Optional only because older captures lack them (`run_record.rs:52-55`) — a storage-history reason on the wire |
| **`session.name`** | seed (skill or `word_pair(run_id)`, `human_session.rs:1277-1281`) unless `session-name.json` exists (`:1375-1384`); `rename` → `find_session` (4-way dispatch, `:602-639`) → `write_session_name` under a lock (`run_record.rs:1079-1131`) → re-surface; boundary replacement copies the file (`human_session.rs:1170-1193`) | 2, plus a copy on Run replacement | **F8** the name is a Session attribute stored as a Run-dir sidecar because the Session has no record; `carry_session_name` exists only to keep the sidecar attached |
| **`bind`** | does not exist: `lf/mod.rs:724-779` has List/Open/Complete/Rename/Ready; Work is written only at launch from `--task/--wave/--as` (`lf/mod.rs:107-121` → `lf/commands/run.rs:35,60`), the S5 checkout inference (`lf/commands/run.rs:93-108` → `ops/run.rs:239-266`), or Ask inheritance from the parent manifest (`human_session.rs:428-434`) | — | **F9** the manifest is written once; the only post-launch mutation precedent is the name sidecar. Jack rejected a `session-work.json` sidecar |
| **`task.runs`** | `runs --task <identifier>` (`RegistryQuery.swift:217-219`) → `collect_runs` (`lf/commands/runs.rs:66-71`) → `scan_runs_since` reads **every** manifest under `runs/` (`run_record.rs:513-535`) → `WorkCatalog::load` opens SQLite read-only and builds per-Work selector lists (id, subject, external id, plus parents; `work_catalog.rs:55-98`) → `resolve_run` picks the first `task:`/`project:`/`wave:` subject and matches strings (`:100-135`) | 5 | **F10** a directory scan plus a string-selector catalogue to answer a foreign key; the Run has no `task` column so the catalogue reconstructs one per call. `TaskSnapshot.runs` (`ops/task.rs:88`) is a second inclusion path |
| **`run.task` written at launch** | `WorkBinding.subjects` = three strings (`ops/run.rs:187-191`) → `begin_run_capture` maps them to `SubjectAttribution::declared` (`lf/commands/run.rs:762-776`) → **also** `record_task_start` parses `task:` back out and writes a `task_events.started` row (`:818-836`, called at `:604`, `:666`) | 3 | **F11** the Run→Task fact is written twice, once as a string on the Run and once as a Task event, because the Run cannot be queried by Task. `started` then folds `task_events`, `task_flow_positions.worker_generation`, and PR publication (`store/sqlite/chapters.rs:97-106`, `lf/commands/waves.rs:1090-1093`) |
| **`task.new_session()`** | `newTaskSession` resolves the worktree or runs `lf task prepare` (`SessionsView.swift:506-531`) → `[lf, --interactive, --task, ISSUE, :, prompt]` (`ConversationLaunch.swift:32-38`) → new shell pane → `resolve_work_selection` requires an unmerged `task_prs` row (`ops/run.rs:172-176`; a prepared Task has one) → subjects → manifest → back through F1 to be listed | 5 | Task identity round-trips TaskId → issue identifier → argv → `task:` string → resolver → TaskId. "Active PR" means an unmerged `task_prs` row, not the GitHub object Jack pictures |
| **`task.flow` / start / resume / restart** | `TaskFlowSnapshot{recommended, record, controls}` on the roadmap Task (`ops/task_flow.rs:13-19`; `PinnedTaskFlow` `:35-48`) → `lf task run/restart/resume` (`RegistryQuery.swift:192-204`) | 1 | Maps one-to-one. This is the shape the rest should look like |
| **`task.worktree`, `active_pr`, branch → Task** | `Task.worktree` (`work/task/mod.rs:612-629`), `task_prs.branch` → `task_by_branch` (`store/sqlite/children.rs:472-490`) → `task_for_checkout` (`ops/task.rs:1064-1076`) | 1 | Natural |
| **`task.comments`** | `pm task comments --id --wave --json` → Linear paginated read (`RegistryQuery.swift:209-210`) | 1 | Natural, read-only |
| **`task.changed_since`** | no API. Pieces: `TaskSnapshot.latest_event` (`ops/task.rs:97`), `lf task changes` = file diff (`:117-123`), comments, `runs --task`, `TaskEventKind::Steer` (`work/task/mod.rs:648+`). The app polls `session list` (`PodiumView.swift:100`) and the roadmap | — | Nothing records "looked"; the events exist, the cursor does not |
| **`session.provider`** | interactive: manifest `harness`; Flow: `recorded_provider(local run dir)` else `None` on a remote Home (`human_session.rs:1729-1733`, `:1392-1395`) | 1 | **F12** `provider: Option` and `title_source: Unavailable` exist because of Home locality (`run_record.rs:1034` comment), not because the user's Session can lack a provider |

### Hop catalogue (the flags, collected)

- **F1/F9/F11** — Run→Task is a prefixed string in a ranked list with a
  Declared/Inherited source, immutable after launch, and mirrored into a Task
  event so it can be queried. Jack's model: a nullable field on the Run.
- **F8** — Session name is a sidecar with a copy step because there is no
  Session record; Jack's model: a name with provenance on the Run.
- **F5/F6** — Swift regroups Sessions under Tasks per body and reads the Task
  back out of the grouping instead of the record's own `work`.
- **F10** — `runs --task` scans all manifests and reconstructs a selector
  catalogue from SQLite on every call.
- **F2** — a read path reuses the launch resolver and inherits its
  active-PR guard; a merged PR makes a bound Session look unbound.
- **F3/F4/F7/F12** — wire fields that exist for storage or locality reasons:
  `wave_id`, `work_path`, the `project:` subject, Optional `node`/`iterations`,
  Optional `provider`, `title_source: unavailable`.
- Four Session stores and three id schemes behind one `session list`; a
  second `runs` table in SQLite that already has the Task FK the manifest
  lacks, and no writer in non-migration code that I could find.

## Part 4 — The natural model `[proposal]`

One record: **Run**. A Session is a Run's conversational face; nothing else
has a store.

```text
Run (manifest.json, updated in place under the lock write_session_name uses)
  id, created_at, harness, model, skill, cwd, worktree, surface, host
  task:   Option<TaskId>           ─┐ Jack: "Run.task = Session.task"
  wave:   Option<WaveId>           ─┘ set together; wave alone for Wave-scope conversations
  work_source: declared | checkout | inherited | bound | none
  step:   Option<RunFlowStep>       (unchanged: flow, invocation, node, iterations)
  name:   Option<{title, source: generated | human}>   (was session-name.json)
  launch, context, runtime_*        (unchanged)
```

- **Session = Run projected**: `session list` = Runs with a conversation
  (interactive history, or a human boundary whose `session_run_id` is this
  Run). `id == run_id` for every kind. `task`, `wave`, `provider`, `name`,
  `step` are read off the record; `work_path` and `wave_id` are computed in
  the client from the roadmap it already holds, or dropped.
- **One reader**: `Run.task` answers `session.task`, `runs --task`,
  `task.sessions` (Rust gains `session list --task ISSUE` and the Swift
  grouping becomes a lookup by field), and `run.task` in Recent runs. The
  `WorkCatalog` string matcher is not needed for the Task filter; Wave and
  Project filters read `Run.wave` and disappear for Project.
- **Bind** = write `task`/`wave`/`work_source=bound` on the Run; membership,
  name, id, panes untouched, which is exactly the S6 invariant.
- **Started** can be derived as "any Run with `task = X`" plus the existing
  Flow and PR evidence, so `record_task_start` and the duplicate
  `task_events.started` can go once `runs` is indexable by Task (a small
  per-Home index, or the SQLite `runs` table finally getting written).
- **Deleted**: `SubjectAttribution`, `AttributionSource`,
  `preferred_work_selector`, `attributed_work`'s resolver round-trip,
  `session_wave_id`, `session_work_path`, `session-name.json`,
  `carry_session_name`, the `project:` subject and `WorkRef::Project` in Run
  attribution, `WorkCatalog` selector matching for Runs, Swift
  `subject(for:)`, the per-body `WorkspaceProjection` rebuild (cache keyed by
  readings identity), `AskSessionRecord.work_selector`.
- **Kept**: `RunFlowStep` and `SessionFlowMembership` (already one shape),
  `TaskFlowSnapshot`, `task_by_branch`, the Linear comments read.

**Migration, one paragraph.** Old manifests have no `task`, `wave`,
`work_source` or `name`; they decode to `None`/`none`, and readers ignore the
old `subjects` and any `session-name.json` beside them. No shim reads
`subjects`; a Run launched before the change is unbound and generated-named
until someone binds or renames it. `runs --task` therefore shows no
pre-migration history, which is a loss Jack must accept or reject (question 2).
The DTO fixtures for `session*.json` lose `wave_id`, `work_path`, `provider`
as Optional and gain nothing; Rust, Swift and the fixture move together.

## Part 5 — S5 and S6 on that model `[proposal]`

**S5 (checkout binding).** At interactive/headless launch with no
`--task/--wave/--as`, `resolve_checkout_binding` writes `task`, `wave`,
`work_source: checkout` on the new Run. It stops going through
`resolve_work_selection`'s active-PR guard: if `task_for_checkout` finds a
Task, bind to it; a landed branch still binds, since the checkout *is* that
Task's worktree (Q8 first half — see question 4). The Started write stays
until `started` reads Runs, then it goes. Explicit `--task` writes
`work_source: declared`. Proof stays the existing isolated CLI test plus one
assertion that `session list --json` and `runs --task` read the same field.

**S6 (bind and the room).** `lf session bind <run-id> --task ISSUE |
--wave NAME | --none [--json]` updates the Run record in place and returns the
Session. Legality is a `Bind` action beside Open/Complete: available for any
Run with a conversation whose `step` is not a human boundary of a Task Flow;
Flow-boundary Sessions carry `unavailable_reason: "this Session's Task is its
Flow position"`. The orphan section is `Runs with a conversation, task = null,
wave = null`; the control room is the existing multiplexer keyed `orphans`
with one `.session(run_id)` pane per orphan and Bind at rest, as designed. No
Swift path matching; no fold-under: after a bind the field is authoritative and
the row moves because the projection reads it. `AskSessionRecord` inherits by
copying the parent Run's `task`/`wave` into its own Run with
`work_source: inherited`.

## Questions only Jack can answer

1. **One id scheme?** Interactive Sessions are Run ids; Flow review Sessions
   are `task:invocation:flow:node:iteration`; Asks are `ask_<uuid>`. On the
   natural model every Session is its Run. Do Flow and Ask Sessions become
   Runs (their `session_run_id` already exists), or stay three kinds behind
   one list with three stores?
2. **Old Runs.** Null Task forever, or a one-time rewrite from `subjects`
   so `runs --task` keeps pre-migration history? The doctrine says no shims;
   a rewrite is not a shim but it is a history edit.
3. **Where does `started` come from?** Derived from Runs (any Run with
   `task = X`), or kept as a Task event written at launch? The first deletes
   `record_task_start`; the second keeps the double write.
4. **Bind to a completed or landed Task.** The flexibility doctrine says no
   guard; today's Started write refuses on a done Task and would fail the
   launch. Bind and skip the Started write, bind and refuse, or leave unbound?
5. **Wave on the Run.** Store both `task` and `wave`, or `wave` only when
   `task` is null? Both is one read for the sidebar; wave-only-when-null is
   smaller.
6. **The SQLite `runs` table.** It already has `source_kind`/`source_id` and
   nothing writes it outside migrations. Is it the index the manifests need,
   dead weight to delete, or the Wave-runtime's own concept that must stay
   separate?
7. **Name on headless Runs.** With `name` on the Run, a headless Run can carry
   a skill-seeded name too. Wanted, or Sessions only?

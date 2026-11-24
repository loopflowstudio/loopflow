# Code, persistence and public APIs

Part of the [Architecture Reference](reference.md).

## Code territory and rough size

These are historical navigation estimates from before the execution-owner conversion,
rounded to the nearest hundred; they are not a measurement of the current diff.
They include inline tests and comments; migration SQL and external test trees
are listed separately. The counts are navigation aids, not quality metrics.

| Territory | Main paths | Approx. LOC | What lives there |
| --- | --- | ---: | --- |
| CLI and presentation | `rust/loopflow/src/lf/`, `src/bin/` | 31,700 | Clap grammar, command dispatch, status/read models, terminal output |
| Operational workflows | `rust/loopflow-execution/src/ops/` | 25,800 | Task control, sessions, PR, Git, release, planning operations |
| Prompt assembly, Graph runner and provider processes | `rust/loopflow-execution/src/prompt/`, `agent/`, `graph/`, `skills/`, `src/harness/` | 29,300 | Skill/Graph discovery, prompt assembly, provider subprocesses and streams |
| Tracked Work | `work/`, `pm/` | — | Wave/Task facts, Task delivery identity, tracker models |
| Storage and command journal | `store/`, `journal/` | 19,700 | SQLite access, migration runner, durable rows, outer command receipts |
| Authentication and history | `provider_auth/`, `transcript/` | — | Service authentication, native homes and transcript adoption; native clients own subscription credentials |
| Harness formats | `rust/loopflow-harness/src/` | — | Stream decoders, transcript readers and file locations for each native client; no store |
| Shared root modules | top-level `src/*.rs` | 10,400 | Session captures, artifact switching, repository identity, subscriptions |
| Migration SQL | `store/migrations/*.sql` | 4,900 | Immutable released batches and merged slug migrations, plus a branch's unmerged one |
| Swift app production | `swift/Loopflow/`, `swift/LoopflowMac/` | 18,200 | Shared DTOs/services and macOS UI |
| External Rust/Python/Swift tests | `rust/loopflow/tests/`, `python/tests/`, `swift/LoopflowTests/` | 27,100 | Cross-module, wire, migration, CLI, and app proofs |

## Persistence map

Loopflow deliberately uses several stores because no one store owns all truth.

```text
repository files + Git        authored goals, memory, Skills, Graphs, code
Machine SQLite                  planning, delivery, Process and Session owners
immutable input storage      retained input evidence and captured payload files
provider-native homes        model credentials and resumable sessions
Linear / GitHub              shared planning and delivery truth
installation directory    immutable binaries and switch receipts
kernel locks                 live local exclusion authority
```

### Live SQLite tables

| Owner | Tables | Purpose |
| --- | --- | --- |
| Planning | `waves`, `projects`, `tasks`, `task_creation_intents`, `task_comments`, `task_issue_identities`, `task_deletions`, `task_events`, `tracker_actions` | Stored plans and definitions, optional tracker links, stable Work identity, creation records, comments, deletion recovery and the outbound tracker queue |
| Submitted creations | `creation_receipts` | Request IDs, content fingerprints, original resource identities and server-owned outcomes for Task creation and new Session threads |
| Served command answers | `command_answers` | Request IDs, content fingerprints and first answers retained for one day so a lost reply can be retrieved without repeating the write |
| Execution | `agent_sessions`, `session_events`, `session_activity` | Conversations, selected captures, observed provider transitions, native completions, turn receipts and the current observations availability is judged from |
| Graph processes | `graph_processes`, `graph_process_steps` | One append-only row per Graph process and per step it started. A run records its Task (`task_id`) as it starts: the Task it names, else the Task whose checkout is its directory's Git worktree, else the Task of the Session that started it |
| Task graphs | `task_graphs` | The graph a Task took up, the step it stands at and the run carrying it |
| Task on this Machine | `task_checkouts`, `task_primary_sessions`, `task_moves` | This Machine's checkout (worktree path, branch, base commit, parent PR), its primary conversation even without a checkout, and each move it sent or received (`sent`, `arrived`) |
| CLI processes | `processes` | Indexed command lifecycle and immutable causal ancestry, written by command start and completion |
| Task delivery | `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments` | Optional PR, retained historical PRs and tracker events |
| Work adjuncts | `tool_responses`, `task_ownership`, `people` | `tool_responses` is leaving (never had a caller); the append-only Task ownership log (each entry names the Machine that wrote it) and the people it names, keyed by tracker identity |
| Desktop arrangement | `dashboards`, `dashboard_arrangements`, `work_pane_layouts` | Saved Dashboard queries and their occurrence order; Task order is `tasks.planning_rank`; `work_pane_layouts` is leaving, since Desktop keeps pane layouts per Mac |
| Historical Ask | `ask_exchanges`, `ask_linear_comment_outbox` | Retained exchange/publication facts; former Ask rows are ordinary conversations in `agent_sessions` |
| Linear refresh | `tracker_projects`, `tracker_items`, `tracker_wave_sync`, `tracker_events`, `linear_team_checks` | Tracker events, shared attempt freshness and successful team watermark |
| PR landing | `pr_landings`, `ci_incidents` | Exact PR-head delivery intent, claims, and repair admission |
| Rental | `ondemands`, `pools`, `pool_machines`, `rentals`, `machine_records` | Creation receipts, owned machine commitments with their cost and expiry, admitted rentals with caps and deadlines, and records copied back before a machine is given back; templates, availability and admission limits live in personal configuration |
| Machine and provider authority | `machines`, `access_profiles`, `provider_accounts`, `provider_account_limits`, `provider_account_switches`, `provider_session_accounts`, `provider_tokens` | Machine identity (id, public name, person) and service browser identities and credentials, retained historical limits, account switches and delivery receipts; historical account rows do not select native launch credentials |
| Local observation/cache | `blob_tokens`, `github_people` | Deterministic Git-blob token counts and the GitHub user behind each Task owner's email |
| Change observation | `store_revisions` | One counter per displayed domain (`planning`, `sessions`, `graphs`, `processes`, `usage`), bumped by schema triggers inside each writer's transaction. Derived, never authored: usage moves only `usage`, and every other Session history row moves `sessions`. GraphProcess rows move `graphs`, a Task's graph and step `planning`, and a Session's stream reading `sessions` when availability or reported detail could change; quiet arriving is the reader's clock, not a write. Every `--subscribe` read compares it to decide whether to read again |
| Schema | `schema_migrations` | Applied migration identity and checksum frontier |

Migration files on origin/main remain immutable. Three direct draft groups establish
the final Process, Project-status and Session schema. Current Work, links, accounts,
routes and resumable conversations survive the cutover. Migration rehearsal uses
a disposable copy and leaves the installed Machine unchanged.

### Filesystem state

| Location | Contents | Write pattern |
| --- | --- | --- |
| `.lf/skills/`, `.lf/graphs/`, `.lf/config.yaml` | Repository-owned execution definitions | Authored and reviewed with code |
| `.lf/wave/<name>/GOAL.md`, `MEMORY.md` | Wave intent and curated memory | Authored and reviewed with code |
| `.lf/releases/<tag>/<commit>-<run>/` | Prepared release bytes and their candidate receipt | Replace one exact candidate atomically; retain through retry, remove after publication |
| `$LF_HOME/runs/<prefix>/<artifact-key>/` | Session capture manifest, event streams, terminal receipt | Publish once, append streams, settle once |
| Machine provider directories | Provider-native login and resume state | Owned by provider adapters |
| Git directory `loopflow/` receipts | writer/sync/PR mutation coordination | Kernel-locked receipt files |
| installation root | Versioned artifact sets and switch receipts | Stage immutably, select atomically |

### External systems

Task and Project rows own saved planning; Linear mappings link optional tracker
events and pending actions. A local save can leave tracker actions pending;
inbound planning grants no graph movement or Process control. GitHub owns PR
heads, checks and merge. Git owns commits and worktrees. Model providers own their
session and usage semantics; local observations do not replace those authorities.

## Processes and public APIs

```text
interactive shell / automation / Loopflow.app
                  |
                  v
                 lf
       +----------+-----------+
       |          |           |
       v          v           v
 planning APIs   Skill run    Git/PR operations
       |          |           |
       |          v           +---- Linear / GitHub
       |       provider
       |          |
       v          v
 SQLite       LfSession history

Task CLI -> fresh Graph -> one lf process starting each step Process
Wave operation -> finite planning LfSession
Task/Wave-bound helpers -------------> shared conversation admission
```

| Surface | Responsibility | Scope |
| --- | --- | --- |
| `lf <skill>` and `lf graph` | Direct Skill execution and Graph composition | Current process and Machine |
| `lf wave`, `task` | Durable planning and Work coordination | Work resolved in the current planning store |
| `lf session` | Sessions and explicit resolution | Current Machine LfSession state |
| `lf wt`, `commit`, `sync`, `pr`, `repo ci`; `lf scratch` | Worktree and delivery operations | Exact repository/Task/GitHub object |
| `lf session usage`, `lf session history linear`; `lf machine doctor` | Execution and process observation | Current Machine only |
| `lf machine` | Machine identity | Current Machine unless routed explicitly |
| `lf task claim`, `unassign` | Task ownership | The writer Machine, reached over `lf --machine` |
| `lf --machine <machine-id> <args...>` | Run the target Machine's `lf` | Explicit remote Machine; no implicit fan-out |
| Loopflow.app | Swift projections and user interaction | Queries the same DTOs and remote routes; owns no lifecycle |

Most commands are local by default. `lf --machine` is transport, not a second API:
the inner `lf` and separator are implicit, the target re-resolves its own Machine
state, and durable processes scrub foreground-forwarded secrets before
detaching.

<a id="harness-launch-and-run-records"></a>

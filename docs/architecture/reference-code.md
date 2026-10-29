# Code, persistence and public APIs

Part of the [Architecture Reference](../architecture-reference.md).

## Code territory and rough size

These are historical navigation estimates from before the execution-owner conversion,
rounded to the nearest hundred; they are not a measurement of the current diff.
They include inline tests and comments; migration SQL and external test trees
are listed separately. The counts are navigation aids, not quality metrics.

| Territory | Main paths | Approx. LOC | What lives there |
| --- | --- | ---: | --- |
| CLI and presentation | `rust/loopflow/src/lf/`, `src/bin/` | 31,700 | Clap grammar, command dispatch, status/read models, terminal output |
| Operational workflows | `rust/loopflow/src/ops/` | 25,800 | Task/chapter control, sessions, PR, Git, release, metrics, PM operations |
| Prompt assembly, Graph runner and provider processes | `rust/loopflow/src/prompt/`, `agent/`, `graph/`, `skills/`, `src/harness/` | 29,300 | Skill/Graph discovery, prompt assembly, provider subprocesses and streams |
| Tracked Work | `work/`, `pm/` | — | Wave/Task facts, Task delivery identity, planning/provider models |
| Storage and command journal | `store/`, `journal/` | 19,700 | SQLite access, migration runner, durable rows, outer command receipts |
| Provider authority | `provider_auth/`, `provider_account/` | 7,500 | Login, encrypted tokens, account homes, routes and leases |
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
| Planning | `waves`, `projects`, `project_changes`, `project_events`, `tasks`, `task_creation_intents`, `task_changes`, `task_comments`, `task_comment_deliveries`, `task_state_deliveries`, `task_issue_identities`, `task_deletions`, `task_events` | Stored plans and definitions, optional provider links, stable Work identity, creation receipts, comments and deletion recovery |
| Execution | `agent_sessions`, `session_events`, `session_activity` | Conversations, selected captures, native completions, turn receipts and the current observations availability is judged from |
| Graph processes | `graph_processes`, `graph_process_steps` | One append-only row per Graph process and per step it started |
| Task graphs | `task_graphs` | The graph a Task took up, the step it stands at and the run carrying it |
| CLI processes | `processes` | Indexed command lifecycle and immutable causal ancestry, written by command start and completion |
| Task delivery | `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments` | Optional PR, retained historical PRs and provider observations |
| Work adjuncts | `tool_responses`, `task_ownership` | Tool answers and the append-only Task ownership log |
| Desktop arrangement | `dashboards`, `dashboard_arrangements`, `task_orders`, `work_pane_layouts` | Saved Dashboard queries and their occurrence order, the repository's Task order, and portable pane layouts |
| Historical Ask | `ask_exchanges`, `ask_linear_comment_outbox` | Retained exchange/publication facts; former Ask rows are ordinary conversations in `agent_sessions` |
| Linear acquisition | `pm_projects`, `pm_items`, `pm_wave_projects`, `pm_wave_sync`, `pm_issue_changes`, `linear_team_checks` | Provider observations, shared attempt freshness and successful team watermark |
| Metrics | `metric_instruments`, `metric_observations` | Registered producers and accepted measurements |
| PR landing | `pr_landings`, `ci_incidents` | Exact PR-head delivery intent, claims, and repair admission |
| Rental | `ondemands`, `pools`, `rentals`, `machine_records` | Rental templates and limits, lendable owned machines, admitted rentals with caps and deadlines, records copied back from a machine before it is given back |
| Machine and provider authority | `machines`, `access_profiles`, `auth_browser_bindings`, `provider_accounts`, `provider_account_limits`, `provider_account_switches`, `provider_routes`, `provider_session_accounts`, `provider_tokens` | Machine routes, credentials, selection, limits, shared-home account switches, delivery receipts |
| Local observation/cache | `blob_tokens`, `github_people` | Deterministic Git-blob token counts and the GitHub user behind each Task owner's email |
| Change observation | `store_revisions` | One counter per displayed domain (`planning`, `sessions`, `graphs`, `processes`, `usage`), bumped by schema triggers inside each writer's transaction. Derived, never authored: usage moves only `usage`, and every other Session history row moves `sessions`. GraphProcess rows move `graphs`, a Task's graph and step `planning`, and a Session's stream reading `sessions` when availability or reported detail could change; quiet arriving is the reader's clock, not a write. Every `--watch` read compares it to decide whether to read again |
| Schema | `schema_migrations` | Applied migration identity and checksum frontier |

Migration files on origin/main remain immutable. Three direct draft groups establish
the final Process, Project-status and Session schema. Current Work, links, accounts,
routes and resumable conversations survive the cutover. Migration rehearsal uses
a disposable copy and leaves the installed Machine unchanged.

Metric observations retain their original payload and content hash, including the
Wave spelling used by the producer. `MetricObservationIdentity` is that hash input;
`MetricIdentity` is the canonical Work association used by contracts, validation,
qualification and readings. The relational Wave key can change case without
rewriting evidence or changing equal-time selection. Reimporting the original
payload remains idempotent. Metric names and UUID punctuation retain their bytes.

### Filesystem state

| Location | Contents | Write pattern |
| --- | --- | --- |
| `.lf/skills/`, `.lf/graphs/`, `.lf/config.yaml` | Repository-owned execution definitions | Authored and reviewed with code |
| `.lf/wave/<name>/GOAL.md`, `MEMORY.md`, `metrics/` | Wave intent, curated memory, metric contracts | Authored and reviewed with code |
| `.lf/releases/<tag>/<commit>-<run>/` | Prepared release bytes and their candidate receipt | Replace one exact candidate atomically; retain through retry, remove after publication |
| `$LF_HOME/runs/<prefix>/<artifact-key>/` | Session capture manifest, event streams, terminal receipt | Publish once, append streams, settle once |
| Machine provider directories | Provider-native login and resume state | Owned by provider adapters |
| Git directory `loopflow/` receipts | writer/sync/PR mutation coordination | Kernel-locked receipt files |
| installation root | Versioned artifact sets and switch receipts | Stage immutably, select atomically |

### External systems

Task and Project rows own saved planning; Linear mappings link optional provider
observations and pending effects. Local settlement proves no provider delivery;
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
| `lf ops wt`, `commit`, `sync`, `pr`, `ci`; `lf scratch` | Worktree and delivery operations | Exact repository/Task/GitHub object |
| `lf monitor`, `ps`, `top`, `prune`; `lf history usage`, `lf history linear`; `lf self doctor` | Execution and process observation | Current Machine only |
| `lf ops machine` | Machine identity | Current Machine unless routed explicitly |
| `lf task claim`, `unclaim` | Task ownership | The writer Machine, reached over `lf --machine` |
| `lf --machine <machine-id> <args...>` | Run the target Machine's `lf` | Explicit remote Machine; no implicit fan-out |
| Loopflow.app | Swift projections and user interaction | Queries the same DTOs and remote routes; owns no lifecycle |

Most commands are local by default. `lf --machine` is transport, not a second API:
the inner `lf` and separator are implicit, the target re-resolves its own Machine
state, and durable processes scrub foreground-forwarded secrets before
detaching.

<a id="harness-launch-and-run-records"></a>

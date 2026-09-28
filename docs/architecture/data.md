# Data and persistence

Loopflow has no universal database. Each store exists because a different
actor owns the fact: the repository owns authored intent, a Home owns local
planning and execution evidence, Linear and GitHub own shared workflow truth,
and the kernel owns live exclusion.

```bash
lf wave status product --json   # joins planning and provider evidence
lf runs --json             # selects Run rows, then reads each Run's record
lf ps --json               # samples live OS facts
```

These commands do not read three views of one hidden lifecycle. They project
different evidence for different questions.

## Truth map

```text
repository files + Git        authored goals, memory, Skills, Flows, code
planning SQLite               local planning, delivery, Flow invocations and Sessions
Run record files              one Home's provider-launch evidence
provider-native homes         model credentials and resumable sessions
Linear / GitHub               shared planning and delivery truth
machine install directory     immutable artifacts and switch receipts
kernel locks                  live local exclusion authority
```

| Question | Read this first |
| --- | --- |
| What is this Wave trying to do? | `wave/<name>/GOAL.md` and `MEMORY.md` |
| What Projects and Tasks exist? | Linear, through the bounded PM projection |
| What Task boundary should resume? | Work domain state joined to its exact `flow_invocations` row |
| What saved Flow step should resume? | its `flow_invocations` row: cursor, current attempt Run and failure |
| What human input is pending? | `sessions` rows joined to their current Runs |
| Which Runs worked on this Wave or Task? | `runs` rows |
| What did one provider launch emit? | the Run record on the Home that launched it |
| Is a local process moving now? | the OS process table joined to local command receipts |
| Did a PR merge? | GitHub |
| May this rebase begin? | live kernel-held Git locks |
| Which binary will a new process start? | machine-install selection receipt |

An identifier can join evidence across these sources. It does not transfer
authority between them.

## Planning SQLite

The durable store keeps facts needed to resume planning, delivery, placement,
credentials, and provider observations. Task invocations retain their captured
execution and settlement history. Sessions of every kind own their title, feedback,
completion and current Run in SQLite; earlier Runs stay in indexed history.
Task and taskless Flows share the invocation owner and executor. Installed-Home
cutover remains a separate acceptance obligation.

The current application tables group by owner:

| Owner | Tables | Purpose |
| --- | --- | --- |
| Tracked Work | `waves`, `projects`, `project_events`, `tasks`, `task_events` | stable identity, status, progress, comments, interrupts, history |
| Project and Task progression | `projects`, `tasks`, `flow_invocations` | Project operation evidence; managed Task's captured Flow, cursor, claim, and blocker |
| Conversations and execution | `sessions`, `runs` | stable conversation identity, current Run, title and saved feedback; every Run's Session, invocation, Task, Wave, caller, provider, times and end |
| CLI processes | `execs` | one actual lf process, immutable causal parent and agent provenance, command completion; the journal transaction maintains its indexed summary |
| Task delivery | `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments` | serial PRs and provider observations |
| Work adjuncts | `tool_responses`, `work_placements` | tool answers and Home placement; Project/Task correction events live in their Work event streams |
| Historical Ask exchange | `ask_exchanges`, `ask_linear_comment_outbox` | retained earlier request/publication facts; current Ask Sessions use `sessions` |
| PM projection | `pm_snapshots` | bounded Linear reads |
| Metrics | `metric_instruments`, `metric_observations` | registered producers and accepted evidence |
| PR landing | `pr_landings`, `ci_incidents` | exact-head supervision and bounded repair |
| Home and provider | `homes`, `access_profiles`, `auth_browser_bindings`, `provider_accounts`, `provider_account_limits`, `provider_routes`, `provider_session_accounts`, `provider_tokens` | routes, credentials, selection, limits, receipts |
| Local observation/cache | `run_events`, `blob_tokens` | outer command history and deterministic Git-blob token counts |
| Schema | `schema_migrations` | applied migration identity and checksum frontier |

Storage interfaces live under [`store/`](../../rust/loopflow/src/store/).
Released migration bytes are immutable. Draft migrations form a dependency-
ordered development frontier and become released only through the release
workflow.

Project and Task progression lives on the Work records and exact
`flow_invocations`; there is no controller table, provider continuation,
phase epoch, active controller slot, Task writer token, or generic Work lease.
The short advancement claim fences one Flow-position version and uses the
existing process ledger for liveness evidence.
Claimed Task settlement, completion, and blocking commit the Flow change and
its Task evidence in one transaction. Ordinary Task updates write facts, while
Flow settlement only touches the Task timestamp.

The current Task invocation stores the shared `ExecutionCursor` tree in `review_json`;
the current Flow name, step, node and human policy come from its captured graph.
Session discovery joins `sessions` to its current `runs` row without decoding
unrelated invocations. Exact Task execution reads still validate the selected
capture. Completion and restart close the invocation without deleting its
capture, cursor or exact claim. Session completion and boundary settlement share
a transaction; Run replacement preserves the Session title and saved feedback.
Cursor checkpoints retain an existing Session unchanged; reservation, publication
and Ready own updates to its current Run and feedback.
`tasks.current_invocation_id` points at the one invocation that advances a
Task; other Flows launched for the Task name it on their invocation and Runs
without becoming its Flow. Run reservation and publication compare invocation identity, version and
current Run; Ready rejects a superseded Run. Chapter
retirement retains evidence from earlier invocations, while only the current
invocation can hold an active worker claim. SQL `step_index` and
`iteration` remain root projections while historical flat progress is
decoded without replacing its captured definition. Ordinary Flow invocations
use the same cursor and navigation rules, with file ownership instead of a
Task claim or transaction. A direct Flow attributed to a Task owns its own
invocation and cannot advance that Task's managed position.

The Session draft converts captured Task reviews, including selected XOR
children, without reading templates. Old pending Run and feedback columns are
retained under historical names solely for offline import evidence; current
readers and writers use Session rows. `lf session import` stores the other
conversation kinds from an older Home's files, once, gives the converted
Task reviews their names and providers, and stores a row for every other Run
record. Earlier attempts of a review and
captures the draft could not map are reported, not stored.

Store open uses a short OS migration lock around backup plus schema
application. A current schema does not take the database write lock merely to
validate. Promotion tests a candidate against an isolated copy before it
selects new artifacts; see [Homes and processes](homes.md#promote-a-new-artifact).

## Filesystem state

| Location | Contents | Write pattern |
| --- | --- | --- |
| `.lf/skills/`, `.lf/flows/`, `.lf/config.yaml` | repository-owned execution definitions | ordinary reviewed file edits |
| `wave/<name>/GOAL.md`, `MEMORY.md`, `metrics/` | authored Wave intent and evidence contracts | ordinary reviewed file edits |
| `$LF_HOME/runs/<prefix>/<run-id>/` | provider-launch manifest, streams, terminal | publish once, append, settle once |
| current Home `flows/<invocation-id>/driver.lock` | saved Flow driver exclusion only; the Flow's definition, cursor, launch facts, current attempt, failure and completion are its `flow_invocations` row | kernel-held lock, no contents |
| current Home `human-sessions/.<hash>.launch.lock` | Session launch exclusion only; an Ask's question, caller, readiness and answer are its `sessions` row and its Runs | kernel-held lock, no contents |
| provider account homes | provider-native login and resume state | provider adapter owns format |
| absolute Git directory `loopflow/` | writer and rebase receipts | kernel-held lock plus readable JSON |
| machine-install root | versioned artifacts and switch receipts | stage immutably, select atomically |

A Run's identity, parents, provider, times and end are its `runs` row. Its
record holds what it emitted: events, transcript, usage and final answer. A
store that cannot take the row never gates launch; that Run is unrecorded and
does not list.

Flow positions capture skills, XOR routers and every path before execution.
The captured occurrence owns its name and human/decision policy. An ordinary
Flow's active boundary stores only attempt identity, provider binding, completion
and readiness; Session projections and recovery read policy through the cursor.
`ExecutionCursor::finish` owns traversal for ordinary and Task Flows; their
storage owners fence settlement. Advance moves forward, Iterate takes the
authored backward edge, and pass counts describe history without a limit.
Autonomous decisions and routes are candidates until their owning Run succeeds.
Human Flow Sessions project the exact saved boundary. Complete records the
review's feedback before continuation; readiness and provider exit do not
complete it. The next step receives feedback through cursor direction, and a
following loop-decide owns any navigation verdict.

Blocked opens a keyed Ask running `unblock`. Calls at the same invocation,
occurrence and pass join the same Ask or recover its saved completion summary.
Completed Asks retain their answers in SQLite and leave the open Session list. Ask completion supplies evidence for reassessment, never a navigation verdict.
The Session projection and provider Run do not become additional cursor owners.

Older unresolved XOR definitions lack captured router or branch content.
Loading today's sources cannot recover those missing historical bytes. Preserve
the original evidence and require explicit recovery or a new invocation;
a fresh-record test cannot establish safe recovery of that historical state.

## External systems

Linear owns shared Initiative, Project, and Issue planning. GitHub owns PR
heads, checks, and merge. Git owns commits and worktrees. Providers own their
sessions, credentials, and usage semantics.

Local records capture bounded observations required for one decision. They do
not silently become a write-through substitute when an external system is
unavailable.

```text
provider observation
        |
        v
record exact source fact
        |
        v
consume it in one domain transition
        |
        v
refresh before a later transition that needs current truth
```

## Read projections

| Projection | Authority copied | Consumers |
| --- | --- | --- |
| `pm_snapshots` | Linear planning | status, roadmap, Mac app |
| `task_linear_observations` | Linear Issue state | Task reconciliation and delivery guards |
| `task_prs`, `ci_incidents` | GitHub PR and check state | Task delivery and landing supervisor |
| `RunSnapshot` | `runs` rows and each Run's record | runs, usage, Work activity, status, Mac app |
| DTO fixtures under `tests/fixtures/dto/` | Rust JSON wire shapes | Rust and Swift fixture tests |
| migration fixtures | draft migrations and canonicalizer | build, runtime, and release checks |

Projections are disposable or bounded read models. They never grant launch,
Work mutation, credential, Git, or signal authority.

## Consistency by boundary

Loopflow does not attempt one distributed transaction across files, SQLite,
Git, Linear, GitHub, and providers. Each workflow chooses the smallest boundary
that can prove its own transition:

- atomic rename publishes a Run manifest;
- exclusive create publishes one immutable Run record;
- a SQLite transaction advances one durable domain state;
- an OS file lock excludes one local critical section;
- an exact PR head fences check and merge evidence;
- a receipt bridges a recoverable filesystem/SQLite or artifact-switch seam;
- provider ids and timestamps preserve external evidence for later refresh.

When a workflow crosses two boundaries, it records enough evidence to retry
from observed truth. It does not claim an atomic commit that neither system can
provide.

## Failure behavior

| Failure | Meaning | Recovery |
| --- | --- | --- |
| planning store unreadable during ad-hoc Skill launch | enrichment unavailable | launch with repository/cwd and declared subject |
| optional Run stream broken | incomplete telemetry | warn, continue, settle terminal independently |
| unterminated Run record | no terminal proof | inspect OS separately; never infer liveness |
| Linear or GitHub unavailable | shared truth cannot refresh | stop the dependent transition and retry later |
| process dies while holding file lock | kernel releases exclusion | validate stale receipt, retry or explicitly adopt |
| crash across relocator/switch seam | receipt describes incomplete transition | verify current sides, finish or roll back idempotently |
| old binary writes after promotion | prior store or incompatible reused schema | inspect the selected store; replay with the current binary |

## Boundary contracts

- Each fact has one named authority.
- Local projections aid reads and recovery; they do not become fallback truth.
- Run evidence is file-local to the Home that observed it.
- Live process state comes from the OS, never an unterminated record.
- Cross-system workflows record retry evidence instead of inventing a global
  transaction.
- Migrations protect retained planning data; they do not preserve deleted
  internal lifecycle concepts.

## Next

[Execution →](execution.md) owns Run-record writes.
[Codebase map →](codebase.md) maps these stores to source modules and public
surfaces.

# Data and persistence

Loopflow has no universal database. Each store exists because a different
actor owns the fact: the repository owns authored intent, a Home owns local
planning and execution evidence, Linear and GitHub own shared workflow truth,
and the kernel owns live exclusion.

```bash
lf status product --json   # joins planning and provider evidence
lf runs --json             # scans this Home's Run-record files directly
lf ps --json               # samples live OS facts
```

These commands do not read three views of one hidden lifecycle. They project
different evidence for different questions.

## Truth map

```text
repository files + Git        authored goals, memory, Skills, Flows, code
planning SQLite               local durable planning and delivery facts
Wave journal JSONL            conversation and resident event history
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
| What controller boundary should resume? | controller state joined explicitly to current Work evidence |
| What did one provider launch emit? | the Run record on the Home that launched it |
| Is a local process moving now? | the OS process table joined to local command receipts |
| Did a PR merge? | GitHub |
| May this rebase begin? | live kernel-held Git locks |
| Which binary will a new process start? | machine-install selection receipt |

An identifier can join evidence across these sources. It does not transfer
authority between them.

## Planning SQLite

The durable store keeps facts needed to resume planning, delivery, placement,
credentials, and provider observations. It no longer stores a parallel Run,
Invocation, Turn, liveness, context, or usage lifecycle.

The current application tables group by owner:

| Owner | Tables | Purpose |
| --- | --- | --- |
| Planning hierarchy | `waves`, `projects`, `project_events`, `tasks`, `task_events` | stable identity, progress, history |
| Controller automation | `project_controller_state`, `task_controller_state` | Project and Task playheads plus provider continuation |
| Task delivery | `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments` | one active branch/PR and provider observations |
| Work input | `steers`, `tool_responses`, `work_flow_positions`, `work_placements` | corrections, tool answers, playheads, Home placement |
| PM projection | `pm_snapshots`, `observation_outbox` | bounded Linear reads and deferred publication |
| Metrics | `metric_instruments`, `metric_observations` | registered producers and accepted evidence |
| PR landing | `pr_landings`, `ci_incidents` | exact-head supervision and bounded repair |
| Home and provider | `homes`, `access_profiles`, `account_access_profiles`, `provider_accounts`, `provider_account_limits`, `provider_routes`, `provider_session_accounts`, `provider_tokens`, `provider_deliveries` | routes, credentials, selection, limits, receipts |
| Local observation/cache | `run_events`, `blob_tokens` | outer command history and deterministic Git-blob token counts |
| Schema | `schema_migrations` | applied migration identity and checksum frontier |

Storage interfaces live under [`store/`](../../rust/loopflow/src/store/).
Released migration bytes are immutable. Draft migrations form a dependency-
ordered development frontier and become released only through the release
workflow.

Controller rows use Work ids as foreign keys, but Work reads never join them.
Project and Task controller code loads Work and controller state separately.
There is no phase epoch, active controller slot, Task writer token, or Work
ownership lease in the schema.

Store open uses a short OS migration lock around backup plus schema
application. A current schema does not take the database write lock merely to
validate. Promotion tests a candidate against an isolated copy before it
selects new artifacts; see [Homes and processes](homes.md#promote-a-new-artifact).

## Filesystem state

| Location | Contents | Write pattern |
| --- | --- | --- |
| `.lf/skills/`, `.lf/flows/`, `.lf/config.yaml` | repository-owned execution definitions | ordinary reviewed file edits |
| `wave/<name>/GOAL.md`, `MEMORY.md`, `metrics/` | authored Wave intent and evidence contracts | ordinary reviewed file edits |
| `.lf/journal/waves/<name>/journal.jsonl` | conversation and resident events | append-only with crash-tail repair |
| `$LF_HOME/runs/<prefix>/<run-id>/` | provider-launch manifest, streams, terminal | publish once, append, settle once |
| provider account homes | provider-native login and resume state | provider adapter owns format |
| absolute Git directory `loopflow/` | writer and rebase receipts | kernel-held lock plus readable JSON |
| machine-install root | versioned artifacts and switch receipts | stage immutably, select atomically |

Run records are deliberately decentralized. A scan can rebuild the complete
local read model. A future index may accelerate queries, but index failure must
not gate launch and the Run record remains evidence truth.

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
| `RunSnapshot` | Home-local Run-record files | runs, usage, Work activity, status, Mac app |
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

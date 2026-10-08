---
layout: default
title: Architecture Reference
---

# Architecture Reference

This page owns the execution contract, its cutover status and the checked
current-source inventory. [Architecture](architecture.md) introduces the model;
[Execution](architecture/execution.md) and [Data](architecture/data.md) develop it.

## Cutover status

**Implemented model; installed acceptance remains open.** Process is one actual lf process;
AgentSession is a continuable interactive or headless conversation; a Flow is
one lf process and the step processes it starts. Run has no
separate product lifetime. The SQLite owners, shared driver, CLI readers and
coordinated Rust/Swift wire types implement this contract.

The implementation has `processes` and `agent_sessions`, with
captured input and provider outcomes beneath their Session owners. Jack Heart's
2026-09-30 compression decision removes historical import, intermediate draft
schemas and old format readers. The cutover retains current Work and links,
accounts/routes and resumable conversations. Configured acceptance is separate.

Current CLI examples use supported spellings. `lf session connect` has `open` as
an alias; `--replace` stops owned clients and connects through the same live
Codex engine, preserving its active turn and other conversations. There is no
separate Session engine-restart operation. A Flow whose driver died leaves its
Processes as history; nothing resumes it. `session list` supports
`--interactive false`, `--history`, `--task` and `--search`; `--all` means all
repositories. `--page --json` uses stable ID pages; Desktop retains earlier
observations until enumeration succeeds. `lf flow list/show --processes` reads
Flows from their driver and step processes. Historical `lf mon show` and `lf replay` remain command spellings.
History exposes Session event and provider evidence; `RunSnapshot` and Session
`run_id` are removed across Rust, Swift and fixtures. Retaining these names in a command or
wire reference does not make Run a target owner. Coordinate migration with
external consumers before changing those references.

### Implementation and acceptance evidence

| Contract | Current evidence and next dependency |
| --- | --- |
| One Process per actual lf process | Ordinary commands record process ancestry and outcome. Help, rejected arguments, screenshot and installation entry use an existing compatible ledger without initialization; unavailable storage remains an explicit gap. Public discovery and Rust/Swift wire fixtures cover the current shape; installed startup acceptance remains open. |
| Stable AgentSession with separate driver and engine | Admission/publication and Session readers use AgentSession directly; ordinary automatic retry retains both outcomes with zero Run rows in the native fixture. Public Codex connect/client replacement preserves the active turn and sibling in the synthetic-Responses proof. Complete configured account/native Machine, stale-client exclusion and shared-engine preservation across every provider path. |
| Flow reads each step's result from its Process | A step's result is its process exit; a deciding or routing step answers through the Session turn its Process captured. Structured native results close the earlier Codex decision-retry transport failure in the real-Codex/synthetic-Responses fixture; configured-provider acceptance remains unproven. OpenCode batch and managed launches share native user-message selection/history and permission ordering; its public retry fixture preserves earlier-caller rejection. The separate Task provider launcher and retired Run owners are deleted; configured recovery across providers remains open. |
| One started Flow is one lf process and its step processes | Jack Heart's 2026-10-05 decision: a Flow adds nothing on top of running `lf flow ...` in a Task worktree, and its record is the lf binary's ordinary logging. Jack's later 2026-10-05 statements put that logging in the Flow's driver: an append-only FlowProcess row with the Flow's name and compiled graph, and one row per step it starts. The driver holds one cursor and its return counters in memory; steps are plain commands that know nothing of their Flow. Configured acceptance remains open. |
| Complete recovery | Six standalone native fixtures use real Codex with synthetic Responses/private Machines, including driver/engine loss and automatic retry. Managed dispatch has synthetic successor-history proof; configured managed provider/account continuity remains unproven. |
| Current-state cutover | Three direct migration groups retain current database state. Release conversion must also recover resumable native identities from a frozen filesystem snapshot. Finished history and intermediate branch schemas are discarded. |
| Indexed discovery and usage | Runs/usage/telemetry/activity and landing conclusions select AgentSession input history before payload decoding, preserving per-input attribution and windows. Native receipts without a captured input/start remain discoverable with unknown ownership and partial usage coverage. Debug measurements at `6d76f74926` used 20,000 Sessions, 5,000 Flows and 100,000 Processes: bounded warm CLI reads were 291–313 ms against a 300 ms empty-store baseline. The broad Process search miss was 362 ms, with a separate 64 ms SQL control. These are uncontrolled-cache local measurements, not release or Desktop latency. History uses captured event sequences and exact native references. |
| Desktop and wire agreement | Off-roadmap ancestry and pane/draft retention pass unit and mounted native-terminal fixtures. Graph and membership wire use captured numeric IDs in Rust/Swift. History and usage use the coordinated Session wire types. Configured Desktop continuity remains unproven. |
| Status-owned Chapters | Focused fixtures cover rotation/workflows, populated current-state adoption, partial/competing-plan preservation and second-Machine convergence. Configured Linear rotation remains unproven. No Chapter table or packet belongs in the model. |
| Integrated acceptance | Affected checks, configured provider/Desktop, backed-up current-state conversion and final consistency remain required. Branch fixture passes do not establish installed acceptance or promotion readiness. |

Admission preserves saved executable/Machine/database handoff and failed-command
diagnostics. Process owns command outcome and error; Session events own provider
outcomes. Exact PID/start receipts establish process identity. Passive readers
acquire no process or Flow authority. Configured-provider and Desktop acceptance
remain open; fixture results establish only their exercised boundaries.

## The system grows outward from a direct Skill launch

The direct Skill launch is the kernel of Loopflow. It remains useful with
no Wave, Project, Task, or daemon. Agent admission requires its Machine's writable store. Higher layers
supply composition, durable context, delivery, placement, and views around its
discovery, prompt, provider, harness, and evidence components. Their
boundary executors remain domain-specific because their settlement rules differ.

```text
                        +--------------------------+
                        | UI and read projections  |
                        | status / roadmap / app   |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | multi-Machine placement     |
                        | MachineId / lf ssh          |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | Task delivery            |
                        | worktree / PR / CI       |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | Task Flows               |
                        | one lf process per Flow  |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | tracked Work + Flows     |
                        | durable facts / inputs   |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | one Skill run            |
                        | discover -> prompt       |
                        | -> route -> spawn        |
                        | -> record -> settle      |
                        +--------------------------+
```

- **Skill runner:** execute one reusable instruction set through one provider
  harness and leave local evidence.
- **Flow composition:** sequence Skill and mechanical Command nodes, route Xor
  branches and follow backward edges. Every step is autonomous.
- **Durable planning:** preserve Wave, Project, and Task intent across crashes;
  each Task Flow is a fresh driver process and the steps it starts.
- **Task delivery:** attach one active remote branch, worktree, and PR to concrete Work.
- **Multi-Machine placement:** run the same commands on a selected machine through
  explicit `lf ssh`.
- **Surfaces:** derive CLI and Mac views from planning facts, provider truth,
  local process observation, and command/conversation history.

The complete system has four kinds of state:

```text
                         external truth
                  Linear       GitHub       providers
                     ^            ^              ^
                     |            |              |
user / automation -> lf -------- domain APIs ----+
                     |
          +----------+-----------+
          |                      |
          v                      v
 tracked Work + delivery    execution evidence
 Wave -> Linear Project -> Task          Process / AgentSession
       stable Work           indexed history + payloads
          |
          v
 repository + Git worktrees

exact local races use OS locks; remote execution uses lf ssh
```

- **Tracked Work** records purpose, input and convergence; each Flow's driver
  holds its position and advances it.
- **Execution evidence** keeps command outcomes, provider turns and Flow results distinct.
- **Delivery** coordinates worktrees, commits, PRs, CI, and merge.
- **Machine authority** places Work and scopes local credentials, processes,
  files, and locks to a Machine.

An identifier in one area is not authority in another. The detailed boundaries
below are the architecture's central constraint.

## Planning and execution vocabulary

These are the accepted product owners; see [cutover status](#cutover-status)
for the current implementation boundary.

```text
Wave --< Linear Project --< Task --< Flow (driver Process)

Process --< Process                 one row per actual lf process, causal edges
AgentSession --> Process         nullable current driver, with generation fence
AgentSession --< history      immutable provider outcomes and usage
driver Process --< step Process     FlowProcess: the driver's graph and each step's node; result is the step's exit
```

| Concept | Owns |
| --- | --- |
| Repository | Wave membership and repository-wide Project rotation |
| Chapter | No stored object: the shared name of each Wave's In Progress Project |
| Wave | Enduring objective, memory, cadence, budget and metric instruments |
| Project | Linear status, Tasks, KRs, targets and the workflow |
| Task | Worktree, serial PRs and every associated Session, Flow and Process; no Flow is privileged |
| Flow | Reusable authored graph of agent/mechanical/router nodes; a running one is a driver Process and its step processes |
| Process | One lf process's immutable causal ancestry and command completion |
| AgentSession | Conversation identity, title, feedback, native thread and provider history |
| Machine | Store, payloads, credentials and exact local process authority |
| Placement | Where Work executes; no authority over merely observed processes |
| Steer | Ordered authored correction to Work |

## Definition and execution projections

`WorkflowDefinition` and `FlowDefinition` resolve independently; same-name local
files override only their own kind. `WorkflowCatalogEntry` carries a Workflow,
while `FlowCatalogEntry` carries its compiled graph and `FlowComposition`
disclosure tree. Invalid sources remain visible with their error.

`lf project workflow list/customize` handles reusable definitions; `show/set`
addresses a Project by durable ID, provider ID or unique name/slug. Task
`workflow show/restart` handles its captured instance. Restart is the existing
move to `start`, preserving graph, moves and execution history.

Task status and roadmap separate `workflow_name`, `latest_flow_process`,
`execution` and `run_control`. The latest `FlowProcessDetail` is ordinary
execution evidence, including completed graphs, not a second Task lifecycle.
Task work and watch carry `flow_processes`. Session membership references the
exact `flow_process_lfid`; Workflow moves retain their Task-run `process_lfid`.
Persisted graphs and historical capture formats remain unchanged.

## Core models and APIs

Constructors and mutation APIs validate within the writing transaction. DTOs
carry typed ancestry; CLI names and issue identifiers resolve at the boundary.
There is no replacement Request, Execution, SkillInvocation, AgentProcess or generic
attempt object. History entries have stable references, not independent lifecycles.

| Owner | Authoritative fields and operations |
| --- | --- |
| `processes` | Durable `lfid`, nullable Unix `pid` (reusable, never identity), immutable `parent_process_lfid`, incoming direct/agent bit and calling AgentSession/provider generation when known; command, cwd, start/end and observed outcome/exit/signal; admit, finish, filter/page |
| `agent_sessions` | Stable ID, purpose and independent interactive flag; title/provenance, request/feedback, typed Task/Wave ancestry, native identity, nullable driver Process and separate driver/provider generations; reserve, connect, bind and rename; historical feedback/completion retained |
| `tasks` | Project, issue, durable disposition, worktree/delivery facts and set-once `started_at` |
| `projects` | Wave, stable Linear Project identity, status, shared chapter name, Flow and planning facts |
| `waves` | Stable repository identity; authored objective/memory/instruments stay in repository files |

### Conversation and driver lifetime

Every agent conversation is an AgentSession: skills, inline prompts, helpers,
reviews, interactive and headless work. Default views select interactive
conversations. `lf session list --waiting` selects conversations Waiting on a
person, using the same projection as Desktop: the driver that owns a provider's
stream saves its latest reading (`session_activity`), and a conversation is
Waiting on an unanswered question, or with no unresolved tool call once an
interactive turn is handed back or the stream has been quiet for 120 seconds.
No reading, or one from a driver that has let go, is unknown. Explicit filters expose headless and
completed history; `--all`
continues to mean all repositories. Interactive mode grants neither review
completion nor Flow authority.

A primary Session is the one ongoing conversation of a scope. It is an ordinary
interactive conversation whose row names that scope; a repository and each
Wave have at most one uncompleted primary. The repository's needs no Wave,
Task or planning provider. `lf session ensure` finds or admits it and starts its
terminal once. `lf session replace` stops the predecessor's provider, then
completes it and admits the successor in one transaction, so a scope never has
two or none midway. Primary grants no Flow, review or process authority, and
the Wave's goal and memory reach it as ordinary Wave context.

A Task's primary is one of its own conversations, named by
`tasks.primary_session_id`; its row carries no scope mark and it stays a Task
member. `lf session ensure --task` keeps an explicit choice, else selects the
sole unfinished interactive conversation, else the most recently used by
`lf resume`'s ranking, and creates one only when the Task has none.

An AgentSession can have many historical driving Processes and at most one current
driver. Driver compare-and-set increments its driver generation. A continuing
engine keeps its provider generation and origin through handoff and a driverless
interval. Old clients may display events but cannot start/steer turns or write
Session state. Passive connection acquires no claim. Dispatch fences include
queued native RPCs and approval replies, not only database claim updates.

Headless admission records a provider-generation reservation in Session history.
A launch records its spawn request under the exact driver fence before starting
any provider process. An unconsumed reservation permits retry after pre-spawn
failure; it is not engine-exit evidence. Once spawn is requested, a missing PID
remains unknown. Saved native thread identity is loaded before account selection.
Historical generations without this evidence retain their liveness protections.

Native dispatch and driver claim, release, and exit share a Session-scoped OS
lock beside the canonical database path. Driver validation releases the SQLite
mutex before provider I/O; history and other Sessions keep using the database.
Lock acquisition has an OS-clock deadline independent of the provider reactor.
Lock files retain their inode across process exit; deleting a live lock file
would let two processes own different locks for the same Session.

Connect transfers the driver while retaining the live conversation. Client
replacement claims the driver before stopping the exact old clients; it leaves the engine and sibling
conversations running. Session resume can retain the recorded native conversation on
a new engine after confirmed engine exit. Missing process evidence remains unknown.
For a released driver without a process identity or connection, failed admission
records the current machine and boot identity against that exact generation and
its originating host. A later admission after a restart of the same machine may
replace the engine while preserving its unknown outcome and native thread. The
observation must precede the restart; wall-clock age, a failed Process and a missing
PID are insufficient. A changed driver invalidates the observation. This fallback
does not restart the host, complete a turn, or settle a Flow.
PID/start identity and native endpoint are operational evidence; conversation
identity, causality and elapsed time grant no signal authority.

An exact driver exit closes its Codex engine and writes a Session receipt under
the same transaction as driver transfer. An old driver's exit cannot stop a
transferred engine. Shutdown verifies the recorded PID/start and process group;
an engine serving other loaded conversations is left running with a close error.
Closing clears the live endpoint and retains the native thread ID and history.
Native terminal providers own their own process exit. An observed
normal or interrupted exit retires an unbound, non-primary conversation only
when that driver owns its provider engine. Task/Wave conversations, primary
Sessions, Asks and Flow reviews remain available. A stopped turn or missing
process never completes a conversation, and retirement never settles a Flow.
Completed Sessions retain their history but contribute no current rows or counts.
Terminal Tasks cannot acquire current review obligations;
independent unresolved Asks remain discoverable.

AgentSession history records provider starts, successful/failed/interrupted
outcomes, retries, durations and usage, correlated with driving Process and native
turn/receipt. Repeated receipts are idempotent. Missing measurements differ from
zero; cumulative samples are not added together. Later continuation never rewrites
an earlier completion a Flow already read. Provider completion and Process
completion are distinct: subsequent command work may fail after the agent succeeds.

### Process lifetime and causality

One actual lf process gets one Process, including nested direct and agent-issued
commands. In-process wrappers reuse it and cannot settle it early; each Flow
step is a child process with its own Process.
The incoming `via_agent` bit describes the caller, not whether the command later
launches an agent. Agent-issued children resolve stable Session/provider-generation
provenance to the current matching driver once, at admission. Delayed children
of a replaced provider retain their historical origin; existing parents never change.

Process outcomes are succeeded, failed or interrupted when observed. Unobserved
completion, exit code and signal stay unknown. The outer command owns terminal
settlement, including interruption. Bootstrap/installation logging must not open
or migrate an incompatible installed store before its authority preflight.
Help, parser errors and installation/screenshot entry paths append process evidence
only to an existing compatible ledger, without creating a Machine, seeding branch
data or applying migrations. An attached ledger stays pinned through command
completion. Missing or incompatible storage leaves an explicit recording gap;
inspection and screenshot cleanup still work. Agent admission requires its Process
and conversation rows before provider launch, even when earlier observation failed.

### Flow outcomes and authority

A Flow is one lf process, the `lf run <flow>`, `lf <flow>` or
`lf task run ISSUE [FLOW]` process, and the step processes it starts as child
processes. Its ID is the Flow process's. Task and taskless execution share the
driver. A Task observes every AgentSession, Flow and Process associated with its
checkout, plus explicitly bound work.
The shared Rust association reader includes checkout descendants at path component
boundaries and retained paths after removal. It neither follows causal ancestry
nor rewrites recorded work or usage. Session inventory exposes `task_ids` so
Desktop navigation uses the same membership as Task status.

No Flow is privileged: every one naming the Task, or run in its checkout,
is equally its work. `task status`'s `work` contains all Sessions, Flows and
Processes, including headless and closed history. Its `execution` and Task Flow
record observe the most recently launched Flow only: `none`, `latest` or
`finished`. The one control is `start`, available whenever the Task can run,
even when an earlier Flow exists.

Completion, cleanup, abandon and restore treat all Task Flows alike. Only live
or unresolved execution blocks: a live driver, a live or unknown step process,
or an unresolved provider turn. A stopped Flow is history and blocks nothing. A
process is never blocked by its own caller lineage or by the Flow whose step it
is. A Process or provider turn that began before the machine’s last boot has
exited; live or unknown execution since then still blocks. Passive membership
grants no control.
Template composition compiles into the graph, which the driver holds in memory
with its cursor. Loop passes are node/iteration positions and lenses over the
step processes. They have no separate lifecycle. A past Flow keeps the graph it
launched with, whatever its YAML says now.

Each step is the command a person would type, started as a child of the driver:

```text
lf --batch [options] skill <name> [message]       # agent step
lf <command> <args>                               # operation step
lf --batch session resume <session> <message>     # correction of an answer
```

A step is told nothing about its Flow and writes nothing about it. The driver
writes FlowProcess, and only by appending: `flow_processes` holds one row per Flow process
(driver Process, Flow name, graph as compiled at launch) and `flow_process_steps` one
row per step it started (child Process, graph node key, per-edge iterations). The
driver finds a step's Process as its own newest child. Running, finished and each
step's result are read from the Processes. Nothing reads the record back to resume,
and no command changes a running Flow; the control is ending its driver.

A step's result is how its process exited. A deciding or routing step gets its
answer contract in its message, and the driver reads the final answer of the
Session turn that step Process captured, accepting a JSON value wrapped in prose
or a code fence. An invalid answer is corrected by resuming the same
conversation: at most two corrective turns, then the Flow fails. After an
operation step the driver looks for a landing of its checkout that is still
being watched; the Flow stops there, and neither failed. Mechanical steps create
no AgentSession.
A conversation driver handoff grants no Flow authority.

A Flow is `current` while its driver has no recorded exit, `completed` when the
driver succeeded and `stopped` when it exited before the last step. A killed
driver leaves dead Processes as history. Nothing restarts or resumes it. The caller,
normally the Task conversation, inspects `lf flow show ID --processes --json` or
`lf task status`, then launches fresh work. Unknown liveness stays unknown.

### Attribution, binding and Started

Task implies Wave. Constructors fill omitted ancestors and reject disagreements.
A Session links to its Flow through the Process that captured its input, whose
parent is the driver, and shares that Flow's nullable Task. `work_source`
is declared, checkout, inherited or bound when known. Historical unknown membership
stays Unknown; absence of evidence does not become Independent.

Bind is write-once: null to a Task, preserving any existing Wave. Same-target bind
is a no-op; reassignment and clearing are unavailable. Done/landed Tasks remain
valid without reopening them. CLI states the permanent target and writes the assignment; Desktop confirms
the exact target before writing. CLI adds no confirmation prompt. The writer compares the selected
conversation/driver and ancestry atomically. Binding cannot alter Flow membership
or bind one member of a taskless Flow inconsistently with its owner.

Jack Heart's 2026-09-30 decision retains prospective usage
attribution: bind records assignment time; earlier usage retains its owner.
The history reader owns this single choice. `lf usage --binds` compares it with
post-hoc attribution from the same history; the
[evaluation](../performance/bind-attribution.md) holds the reading and recommendation. Preserve active-turn start/assignment evidence; unknown allocation remains unknown rather than inventing a token split.
Project/Task moves preserve immutable historical attribution while
validating current ancestry.

`tasks.started_at` is set once when actual agent work first belongs to the Task,
including first bind, or an operation step Process runs in its checkout. General command observation
never starts a Task. Current-state conversion preserves existing Started
timestamps. No later launch or bind moves or clears the timestamp. Chapter retirement also checks authored work, PRs and
Flows: a Task with any Flow has begun. Absent execution evidence alone cannot prove untouched backlog.

### Readers and conversion

One indexed reader per object selects and pages identity, ancestry, command,
skill, title and status before opening payload files. No inventory scans manifests,
sidecars or live PRs to reconstruct identity. Bound conversations remain bound even
when absent from the visible roadmap. Desktop panes key on AgentSession identity,
so bind, rename and driver replacement retain the surface and draft.

Current-state conversion keeps resumable native identities and selected captures.
Finished conversations, old command events and unresolved historical SQL are
not imported. Runtime readers use only the final schema and current encodings.

### Chapters

Linear statuses are the owner: one In Progress Project per Wave, sharing a chapter
name across the repository. Planned Projects express future plans; Completed
Projects retain history. Project `workflow:` is required; a Task with none takes it up.
There is no Chapter row, packet or local switch. Rotation uses an explicit target
and stable Project identities, converges after partial mutations, preserves active
Task identity/worktree/PR/execution, and refuses unrelated competing plans.
A second Machine adopts the same state through ordinary synchronization.

Adjacent APIs keep their own authority: Task PR operations own Git/GitHub;
provider routing owns credentials; Machine placement owns routing; exact process
receipts and current OS evidence own signaling. None follows from causal ancestry.

## Code territory and rough size

These are historical navigation estimates from before the execution-owner conversion,
rounded to the nearest hundred; they are not a measurement of the current diff.
They include inline tests and comments; migration SQL and external test trees
are listed separately. The counts are navigation aids, not quality metrics.

| Territory | Main paths | Approx. LOC | What lives there |
| --- | --- | ---: | --- |
| CLI and presentation | `rust/loopflow/src/lf/`, `src/bin/` | 31,700 | Clap grammar, command dispatch, status/read models, terminal output |
| Operational workflows | `rust/loopflow/src/ops/` | 25,800 | Task/chapter control, sessions, PR, Git, release, metrics, PM operations |
| Prompt and process engine | `rust/loopflow/src/engine/`, `src/harness/` | 29,300 | Skill/Flow discovery, prompt assembly, provider subprocesses and streams |
| Tracked Work | `work/`, `pm/` | — | Wave/Task facts, Task delivery identity, planning/provider models |
| Storage and command journal | `store/`, `journal/` | 19,700 | SQLite access, migrations engine, durable rows, outer command receipts |
| Provider authority | `provider_auth/`, `provider_account/` | 7,500 | Login, encrypted tokens, account homes, routes and leases |
| Shared root modules | top-level `src/*.rs` | 10,400 | Session captures, artifact switching, repository identity, subscriptions |
| Released and draft SQL | `store/migrations/**/*.sql` | 4,900 | Immutable schema history and current draft frontier |
| Swift app production | `swift/Loopflow/`, `swift/LoopflowMac/` | 18,200 | Shared DTOs/services and macOS UI |
| External Rust/Python/Swift tests | `rust/loopflow/tests/`, `python/tests/`, `swift/LoopflowTests/` | 27,100 | Cross-module, wire, migration, CLI, and app proofs |

## Complete ownership map

The following inventory describes **current source during the cutover** and is a
checked ownership index. Transitional input projections and wire names below are deletion
dependencies, not additional target product objects.
Every top-level CLI family, live SQLite table, process entrypoint, HTTP route,
provider, and literal subprocess edge must appear exactly once.

<!-- architecture-map:start -->
| Concept | Truth and authority | Data structure | Persistence | Process owner | Public surface | External edge |
| --- | --- | --- | --- | --- | --- | --- |
| **User** — a person or external harness originating work | User-attributed actions author root input and decide effects that require user intervention. User is actor provenance, not a control credential. | [`Author`](../rust/loopflow/src/durable.rs) | Git supplies `user.name` unless personal Loopflow config overrides it; input records retain source author names. No User row; authored effects persist on the concept they change. | `lf` | `lf :`, `lf desktop`, `lf user` | `process:open`, `process:osascript`, `process:pbpaste`, `process:id` |

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
person, using the same Rust projection as Desktop. `session_activity` holds both
the driver's latest stream reading and an optional terminal-reported snapshot.
A current provider generation's reports take precedence: any blocked record, or
idle for an interactive Session, means Waiting. Other reports and explicit clear
suppress inference. Without reports, an unanswered question, interactive hand-back
or 120 seconds of silence without an unresolved tool call means Waiting.
No current reading is unknown; replacement providers need fresh observations.

Desktop consumes the embedded Ghostty action for every retained surface. Shell
panes keep local records; Session panes publish bounded snapshots through
`lf session observe-status` with the current terminal receipt and provider
generation. Stream identity and increasing sequence fence replacements in SQLite.
An unchanged provider keeps reports across driver handoff. Report text is literal
and display-sanitized; protocol IDs never select Sessions or grant process,
Flow or completion authority. Detached relay observation remains unimplemented. Explicit filters expose headless and
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
| **Skill** — one reusable prompt with assembled context | Repository/builtin Skill Markdown is authoritative; discovery selects one source. | [`Skill`](../rust/loopflow/src/engine/flow.rs), [`SkillSource`](../rust/loopflow/src/lf/discovery.rs) | `.lf/skills/`, builtin Skill files, installed vendor Skill directories | `lf-prompt` | `lf skill`, `lf installation sync-skills`, `lf list`, `lf help` (local command/definition discovery) | `process:python3` |
| **Flow** — definition and one execution | The driver process compiles the FlowDefinition, including Xor paths, and holds the graph and cursor in memory. It records the graph and each step's node in FlowProcess; a step's result is its process exit. | `FlowDefinition`, `FlowComposition`, `FlowProcess`, typed node ID | `.lf/flows/`; `flow_processes`, `flow_process_steps` beside the driver and step Process rows | One CLI driver Process per Flow; each step is its own child process running the plain command | `lf flow`, `lf run` (flow-first definition execution), `lf task run` | — |
| **Workflow** — a Task's nodes, the Flows between them, and where the Task stands | A workflow definition is authored YAML: nodes, where a person takes part in the Task conversation, and edges, each running one Flow. A Task's Workflow is the definition it took up, fixed from then on, and its stored position: at a node, or on an edge with the Process carrying it. Choosing an edge and setting a node are the two writes; the `lf task run` process carrying an edge starts its Flow as a child, again when a Flow process fails, writes the arrival when one succeeds, and otherwise leaves the Task on its edge. Every move is appended with its Process and note. It executes nothing itself. | [`WorkflowDefinition`](../rust/loopflow/src/engine/workflow.rs), [`Workflow`](../rust/loopflow/src/ops/workflow.rs) | `.lf/workflows/`, builtin workflow files; `task_workflows` (one row per Task), `task_workflow_moves` (append-only) | `lf task run` chooses an edge, then starts its Flow as the plain `lf --task ISSUE run FLOW`; `lf task move` sets a node | Task status JSON (`execution.work.workflow`), the Workflow catalog, node guidance in a bound Task launch | — |
| **Wave** — durable operating context with goal, memory, cadence, chat, and project selection | The Wave UUID is durable identity, carried in authored `GOAL.md` frontmatter. SQLite stores a one-segment name and optional parent Wave ID; the readable address is derived through parents within the canonical repository. Directory discovery reconciles names and parents without replacing IDs. `wave/<name>/GOAL.md` and `MEMORY.md` own repository intent; the Linear Initiative owns shared planning membership. | [`Wave`](../rust/loopflow/src/work/wave/mod.rs), [`WaveLocator`](../rust/loopflow/src/work/wave/mod.rs), [`CanonicalRepo`](../rust/loopflow/src/repository.rs), [`WaveConfig`](../rust/loopflow/src/work/wave/config.rs) | `waves`; `wave/<name>/`; an in-flight relocation receipt under `.lf/tmp/wave-relocations/` | Finite Wave-attributed conversations; relocation owns the repository locator lock | `lf wave`, `lf wave list`, `lf wave status`, `lf roadmap`, `lf cron`, `lf discord` | Discord when configured |
| **Chapter / Project** — shared current plan name and one Linear Project per Wave | Linear Project status owns planned/current/completed plans; the repository chapter name is derived from its Waves' In Progress Projects. | `Project`, `ProjectStatus` | `projects`, `project_events`; Linear Project status/content | deterministic convergent rotation from fresh provider facts | `lf project`, `lf project workflow`, `lf repo new-chapter`, `lf repo reteam` | Linear |
| **Live metric** — one reviewed measurement contract owned by exactly one Wave, plus revision-bound current evidence | `wave/<name>/metrics/*.md` owns meaning and Wave ownership; an accepted instrument observation owns its source-time fact; [`MetricPortfolioDto`](../rust/loopflow/src/work/wave/metrics.rs) is the sole derived reading shared across surfaces. Metrics inform KRs but never complete them. | [`MetricContract`](../rust/loopflow/src/work/wave/metrics.rs), [`MetricObservation`](../rust/loopflow/src/work/wave/metrics.rs), [`MetricPortfolioDto`](../rust/loopflow/src/work/wave/metrics.rs) | `wave/<name>/metrics/`, `metric_instruments`, `metric_observations` | Metric instruments write observations; foreground Rust readers derive bounded portfolios. | Status/roadmap JSON, Wave and Task prompts, the shared Swift DTO, and Mac Wave detail expose the same `metric_portfolio`. | — |
| **Task** — concrete work inside exactly one Project | The Linear Issue owns directive/status. The checked-out branch identifies the Task through its active PR; the stored worktree path is placement. Git upstream tracking does not select Task identity. Every Flow process for the Task is equally its work; helpers and delivery commands may mutate the worktree without driving a Flow. Git owns commits/branch state; GitHub owns PR/check/merge truth. | [`Task`](../rust/loopflow/src/work/task/mod.rs), [`TaskPr`](../rust/loopflow/src/work/task/mod.rs) | `tasks`, `task_issue_identities`, `task_deletions`, `task_events`, `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments`; Linear Issue; Git worktree | `lf task run` places the worktree, then drives a fresh Flow in the foreground; foreground operations record delivery evidence | `lf task`, `lf pr`, `lf wt`, `lf sync`, `lf commit` | Linear |
| **PR landing** — one recorded intent to merge an exact PR head | GitHub is authoritative for the PR head, required checks, and merge. One landing generation admits one check at a time; an incident without a recorded response admits one repair. | [`PrLanding`](../rust/loopflow/src/pr_landing.rs), [`CiIncident`](../rust/loopflow/src/work/task/mod.rs) | `pr_landings`, `ci_incidents` | The process holding the claim: `lf pr reconcile` to observe and settle, `lf ci watch` to repair | `lf arm`, `lf land`, `lf pr reconcile`, `lf ci watch`, `lf pr checks` | `provider:github`, model provider for `ci-fix`, `process:git`, `process:gh` |
| **PM projection** — locally readable planning facts | Linear remains authoritative. Repository/provider-scoped Project and issue facts serve both exact Task lookup and Wave views. Wave membership and sync observations reference those shared facts; change receipts invalidate admission without rewriting execution history. | [`PmSnapshotRow`](../rust/loopflow/src/store/mod.rs), [`PmTaskRecord`](../rust/loopflow/src/store/mod.rs), [`PmWave`](../rust/loopflow/src/pm/mod.rs) | `pm_projects`, `pm_items`, `pm_wave_projects`, `pm_wave_sync`, `pm_issue_changes`, `pm_project_name_cutover`, `project_binding_imports`, `project_transitions`, `project_transition_items` | Foreground PM sync and Task lookup | `lf repo`, `lf refresh`, `lf task status` | `provider:linear` |
| **Steer** — correction to Task advancement | Linear comment id/revision; Task identity selects its Runs | [`Steer`](../rust/loopflow/src/durable.rs), [`TaskEventKind`](../rust/loopflow/src/work/task/mod.rs) | Linear Task comments; local Task events cache delivery | Task Runs refresh comments into their starting context | `lf comment`, Linear issue comments | Linear |
| **Tool response** — one idempotent response to a Work-scoped tool request | Stable Work identity plus request id names the response slot; a second, different answer is rejected. | [`ToolResponseWrite`](../rust/loopflow/src/durable.rs), [`ToolResponseReceipt`](../rust/loopflow/src/durable.rs) | `tool_responses` | Store transaction | Internal Work store API | — |
| **AgentSession** — one conversation | Session row owns name, ancestry, readiness, completion and publication. Its current capture references an immutable history event written at reservation. Earlier events retain caller and Work attribution. Complete returns saved feedback; the following typed decision chooses navigation. | `SessionRecord`, `SessionId` | `agent_sessions`, `session_events`; `session_activity` holds driver stream readings and terminal-reported status | Native turn observation retains start/usage/completion; `lf __provider-session` records native identity; Session operations own state | `lf session`, interactive `lf` | — |
| **Machine / Placement / Promotion** — stable machine identity, Work placement, and artifact selection | `MachineId` is identity; SSH route is mutable. Placement is planning state and never process ownership. Promotion owns immutable artifact selection, isolated schema proof, app replacement, and rollback only. Install selects the latest published release independently of caller Git state; the laptop schedule invokes that same command. Checkout updates belong to sync. | [`Machine`](../rust/loopflow/src/durable.rs), [`Placement`](../rust/loopflow/src/durable.rs), [`SwitchReceipt`](../rust/loopflow/src/installation.rs), [`published installation`](../rust/loopflow/src/lf/commands/install/published.rs) | `machines`, `work_placements`; Machine-local SQLite; installation selection and switch receipts; laptop refresh LaunchAgent | The promotion command owns its OS-locked switch transaction | `lf machine`, `lf installation`, `lf ssh`, `lf install`, `lf schedule` | `process:ssh`, `process:launchctl`, `process:systemctl`, `process:/usr/bin/open`, `process:/usr/bin/osascript`, `process:brew`, `process:/bin/sh`, `process:tmux` |
| **Session history projections** — captured events and exact provider evidence | AgentSession owns provider outcomes and Process owns command outcomes; original payload and exact process receipts confer no Flow authority. | `SessionCaptureSpec`, `SessionCaptureManifest`, `SessionHistory`, `ProviderHistory`, `SessionUsage` | Projects AgentSession-owned input/history; Machine-local `runs/<prefix>/<run-id>/` immutable payload and process receipts | shared conversation admission and history | `lf mon show`, `lf replay`, `lf usage`, `lf activity`; Work/status history | `process:lf`, provider harnesses |
| **Browser capture** — one isolated, bounded screenshot transaction | The requested source, viewport, and output name the transaction; only a validated PNG replaces the output. The standalone shell identity and fresh process group keep capture separate from the user's browser and bound to its owner. | [`ScreenshotArgs`](../rust/loopflow/src/lf/mod.rs), [`ProcessGroupGuard`](../rust/loopflow/src/engine/process.rs) | Output PNG only; no control-store state | `lf __screenshot-supervisor` owns one `chrome-headless-shell` process group and observes the public command through a control pipe | `lf screenshot` | `process:chrome-headless-shell` |
| **Process** — one actual lf process | The journal transaction records command completion and fixes each child's causal parent at admission. Agent provenance grants no control authority. | [`ProcessLfid`](../rust/loopflow/src/id.rs), [`AgentCaller`](../rust/loopflow/src/process.rs) | `processes` | Outermost foreground command; installation/bootstrap coverage remains a cutover obligation | `lf monitor`, `lf mon list`; ordinary parsed CLI commands | — |
| **Local process observation** — outer command receipts joined to current OS facts | A live kernel process plus a matching local receipt is observation, not durable ownership. Registered orphan OpenCode groups may be reaped; unclaimed provider PIDs may not. | [`ActivitySnapshot`](../rust/loopflow/src/lf/commands/top.rs), [`ProcessPruneReport`](../rust/loopflow/src/lf/commands/top.rs) | Machine-local Process receipts and OpenCode server registry | The foreground observer samples the process table; no keeper asserts Run liveness | `lf ps`, `lf top`, `lf mon prune`, `lf doctor` | `process:/bin/ps`, `process:ps`, `process:sysctl`, `process:lsof`, `process:kill`, `process:which` |
| **Provider account / route** — credential authority and ordered provider selection on one Machine | Provider token/account rows and Access Profiles own routing; credentials stay in provider homes, encrypted storage, Doppler, or forwarded foreground leases. | [`Provider`](../rust/loopflow/src/provider_auth/mod.rs), [`AccessProfile`](../rust/loopflow/src/profile.rs), [`ProviderRoute`](../rust/loopflow/src/profile.rs), [`ProviderAccount`](../rust/loopflow/src/store/mod.rs) | `access_profiles`, `auth_browser_bindings`, `provider_accounts`, `provider_account_limits`, `provider_account_switches`, `provider_routes`, `provider_session_accounts`, `provider_tokens` | The foreground auth command owns provider login process groups and passive browser handoff; durable processes use credentials installed on their Machine | `lf account` | `provider:claude`, `provider:codex`, `provider:doppler`, `provider:opencodezen`, `process:claude`, `process:codex`, `process:doppler`, `process:opencode`, `process:security`, `process:secret-tool` |
| **Context budgets** — limits and usage for assembled launch input | Existing personal/repo config and Wave frontmatter resolve each limit; the shared prompt assembler measures and enforces it. | [`ContextBudgets`](../rust/loopflow/src/engine/context_budget.rs), [`ContextBudgetReport`](../rust/loopflow/src/engine/context_budget.rs) | Authored config and source files; complete excerpt sources under `.lf/tmp/context/`; no measurement store | Foreground preview and launch assembly | `lf context` | — |
| **Code-size measurement** — repository blobs measured in model tokens | Git blob identity owns content; token counts are deterministic memoized measurements, not Run usage. | [`CodeNode`](../rust/loopflow/src/lf/commands/tokens.rs), [`CodeSnapshot`](../rust/loopflow/src/lf/commands/tokens.rs) | `blob_tokens` | Foreground command only | `lf tokens` | — |
| **Store change revision** — which displayed domain a commit changed | Derived inside the writer's transaction by schema triggers, never authored and never bumped at a call site. Transcript lines move nothing; usage moves only its own revision, which Wave detail follows at most every 10 s. A reader compares revisions to decide what to read again; events only say when to look. | [`StoreRevisions`](../rust/loopflow/src/store/sqlite/revisions.rs), [`WorkFrame`](../rust/loopflow/src/lf/commands/work_watch.rs) | `store_revisions` | Store transaction | `lf monitor work` | — |
| **Schema frontier** — ordered definition of durable control storage | Released migration bytes are immutable authority; drafts join only through deterministic release materialization. | [`Migration`](../rust/loopflow/src/store/migration_catalog.rs), [`MigrationId`](../rust/loopflow/src/store/migration_catalog.rs) | `schema_migrations`; canonical and draft migration files | Store open validates/applies; release cut publishes | `lf release` | `process:sh` (release hooks) |
<!-- architecture-map:end -->

The public API column covers top-level command families, not every subcommand or
Rust function. [`lf` reference](lf.md) owns argument-level detail. DTOs emitted
by `--json` are required-field projections; Rust/Swift fixture tests own their
wire parity.

## Persistence map

Loopflow deliberately uses several stores because no one store owns all truth.

```text
repository files + Git        authored goals, memory, Skills, Flows, code
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
| Planning | `waves`, `projects`, `project_events`, `tasks`, `task_issue_identities`, `task_deletions`, `task_events` | Linear Project statuses, Wave plans, Work identity, corrections, historical evidence |
| Execution | `agent_sessions`, `session_events`, `session_activity` | Conversations, selected captures, native completions, turn receipts and the current observations Waiting is judged from |
| Flow processes | `flow_processes`, `flow_process_steps` | One append-only row per Flow process and per step its driver started |
| Workflows | `task_workflows`, `task_workflow_moves` | The graph a Task took up with its position, and every move, append-only |
| CLI processes | `processes` | Indexed command lifecycle and immutable causal ancestry, written by command start and completion |
| Task delivery | `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments` | Serial PR chain and provider observations |
| Work adjuncts | `tool_responses`, `work_placements` | Tool answers and Machine placement |
| Historical Ask | `ask_exchanges`, `ask_linear_comment_outbox` | Retained exchange/publication facts; former Ask rows are ordinary conversations in `agent_sessions` |
| PM projection | `pm_projects`, `pm_items`, `pm_wave_projects`, `pm_wave_sync`, `pm_issue_changes` | Bounded Linear reads |
| Metrics | `metric_instruments`, `metric_observations` | Registered producers and accepted measurements |
| PR landing | `pr_landings`, `ci_incidents` | Exact PR-head delivery intent, claims, and repair admission |
| Machine and provider authority | `machines`, `access_profiles`, `auth_browser_bindings`, `provider_accounts`, `provider_account_limits`, `provider_account_switches`, `provider_routes`, `provider_session_accounts`, `provider_tokens` | Machine routes, credentials, selection, limits, shared-home account switches, delivery receipts |
| Local observation/cache | `blob_tokens` | Deterministic Git-blob token counts |
| Change observation | `store_revisions` | One counter per displayed domain (`planning`, `sessions`, `flows`, `processes`, `usage`), bumped by schema triggers inside each writer's transaction. Derived, never authored: transcript lines (the `events.jsonl` observation types no summary reader selects) move nothing and usage moves only `usage`; provider attempts under the same receipt key move `sessions`. FlowProcess rows move `flows`, a Workflow's position and moves `planning`, and a Session's stream reading `sessions` when Waiting or reported detail could change; quiet arriving is the reader's clock, not a write. `lf monitor work --watch` reads it to decide which parts to project again |
| Schema | `schema_migrations` | Applied migration identity and checksum frontier |

Released migration files remain immutable. Three direct draft groups establish
the final Process, Project-status and Session schema. Current Work, links, accounts,
routes and resumable conversations survive the cutover. Migration rehearsal uses
a disposable copy and leaves the installed Machine unchanged.

### Filesystem state

| Location | Contents | Write pattern |
| --- | --- | --- |
| `.lf/skills/`, `.lf/flows/`, `.lf/config.yaml` | Repository-owned execution definitions | Authored and reviewed with code |
| `wave/<name>/GOAL.md`, `MEMORY.md`, `metrics/` | Wave intent, curated memory, metric contracts | Authored and reviewed with code |
| `.lf/releases/<tag>/<commit>-<run>/` | Prepared release bytes and their candidate receipt | Replace one exact candidate atomically; retain through retry, remove after publication |
| `$LF_HOME/runs/<prefix>/<artifact-key>/` | Session capture manifest, event streams, terminal receipt | Publish once, append streams, settle once |
| Machine provider directories | Provider-native login and resume state | Owned by provider adapters |
| Git directory `loopflow/` receipts | writer/sync/PR mutation coordination | Kernel-locked receipt files |
| installation root | Versioned artifact sets and switch receipts | Stage immutably, select atomically |

### External systems

Linear owns Initiative/Project/Issue planning shared with the team. GitHub owns
PR heads, checks, and merge. Git owns commits and worktrees. Model providers own
their session and usage semantics. Local rows cache or record observations from
those systems; they never silently become substitute authority.

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
 SQLite       AgentSession history

Task CLI -> fresh Flow -> one lf process starting each step Process
Wave operation -> finite planning AgentSession
Task/Wave-bound helpers -------------> shared conversation admission
```

| Surface | Responsibility | Scope |
| --- | --- | --- |
| `lf <skill>` and `lf flow` | Direct Skill execution and Flow composition | Current process and Machine |
| `lf wave`, `repo`, `task` | Durable planning and Work coordination | Work resolved in the current planning store |
| `lf session` | Sessions and explicit resolution | Current Machine AgentSession state |
| `lf wt`, `commit`, `sync`, `pr`, `ci` | Worktree and delivery operations | Exact repository/Task/GitHub object |
| `lf monitor`, `usage`, `ps`, `top`, `mon prune`, `doctor` | Execution and process observation | Current Machine only |
| `lf machine`, `lf wave place` | Machine identity and Work placement | Current Machine unless routed explicitly |
| `lf ssh <machine-id> <args...>` | Run the target Machine's `lf` | Explicit remote Machine; no implicit fan-out |
| Loopflow.app | Swift projections and user interaction | Queries the same DTOs and remote routes; owns no lifecycle |

Most commands are local by default. `lf ssh` is transport, not a second API:
the inner `lf` and separator are implicit, the target re-resolves its own Machine
state, and durable processes scrub foreground-forwarded secrets before
detaching.

<a id="harness-launch-and-run-records"></a>
## Harness launch and Session captures

The execution cutover uses one AgentSession admission and capture path for Task,
Wave, helper and direct callers.

SQLite owns Process history. Repository trace events live in
`.lf/journal/traces/<trace-id>/events.jsonl` and name their `trace_id` and `process`
node explicitly. Session captures retain the published `~/.lf/runs` directory
as one opaque physical encoding, selected through SQLite artifact keys. Current
and historical captures use the same root: there is no relocation, parallel
layout, alias or privileged conversion. Missing payload does not erase a resumable
Session's SQLite identity. `LF_CAPTURE_KEY` selects subordinate history;
Session/Process caller provenance supplies ancestry and mutation authority.


1. Admit the actual lf Process; resolve typed work without granting Flow authority.
2. Reserve the AgentSession and its initial history/capture reference before
   provider launch. Claim its driver and record exact publication state.
3. Publish immutable input atomically. An unpublished reservation is recoverable;
   uncertain publication/spawn evidence never permits a blind duplicate launch.
4. Start or reconnect the native engine. Record its identity and endpoint, distinct
   from the client's process and the conversation's driver.
5. Append correlated provider outcomes and usage; retain missingness. A Flow's
   driver reads a deciding or routing step's answer from the turn that step's
   Process captured.
6. Settle the actual command's Process when the process completes, independently of
   whether its conversation remains open.

### Retained encoding inventory (LOO-370)

- `session_record::record_dir`, active-reader watches, ablation staging and
  preservation fixtures retain `runs/<shard>/<artifact-key>`. This is the single
  physical capture root; keys (including historical `run_` strings) are immutable.
- `ops/git_operation.rs` keeps serialized `run_id` / `process_lfid` receipt fields
  while Rust names their actual Trace/Process owners. Released SQL, migration
  fixtures and `session_events` historical source labels retain their original
  bytes. Historical cohort readers in `scripts/context_ablation.py` still decode
  `launch`, `run_id` and `parent_run_id` from their frozen inputs.
- `LF_RUN_ID` / `LF_RUN_DIR` occur only in launch scrubbing, rejection fixtures
  and the checksum-pinned released-CLI preservation fixture. Current execution
  reads `LF_CAPTURE_KEY` and typed Session/Process provenance. Capture context does
  not confer Flow or Task settlement authority.
- GitHub workflow/check runs, release-run operations, gate execution receipts,
  launchd `RunAtLoad`, Swift attributed-text runs and ordinary execution verbs
  name other things. Published release notes, dated benchmark reports and chapter
  archives retain historical terminology. The old heading anchor above preserves
  documentation links, not a runtime interface.
- Current Wave JSON exposes `history: Evidence<SessionHistory>`; Rust, Swift and
  `wave_detail.json` share the required field without a fallback. Telemetry uses
  Session metric IDs and labels; ablation/check-cost use capture names. Session
  commands select durable Session/native conversation IDs; monitor's `--input`
  and replay select retained captures.

Production source against `8ea0bec9cf4b0c08ca17c52e57de059000a7b0e3`:
Rust **+545 / −659**, Swift **+64 / −64**, Python package **+0 / −0**,
scripts **+97 / −81** (net **−98** lines). Physical-line comparison includes
comments/blanks, excludes tests/fixtures, SQL, builtin prose and generated output;
Rust test-only attributed items are removed using its syntax tree. Rename pairs
are compared as one file, so moves do not count as deletion. This measures source,
not installed acceptance. Abandoned relocation probes survive in Git at
`6fcdbe9da0b47ef95f1f92ebdb259401a327cd46`; their contrary evidence remains valid.

Payloads may remain large immutable files. SQLite owns identity, attribution,
current control and searchable history. Current capture payloads may use files; ordinary readers select their exact
artifact through SQL, never by scanning retired layouts.
The retained operational process evidence is not a new lifecycle object.

## Tracked Work and Task Flows

```text
lf task run ISSUE [FLOW] -> place the worktree, take up the Project's workflow
                         -> lf --task ISSUE run FLOW as a child: one lf process,
                            started again while a Flow process fails (three at most)
                         -> step processes, one child process each
                         -> current, completed or stopped
```

Every Flow naming a Task is equally its work; none is selected over
another. `task run` blocks and prints. The driver holds the cursor in memory and
advances it from each step's exit; the position a step ran at is on that step
Process's argv.

A driver that dies leaves dead Processes as history. Nothing restarts or resumes
it. Liveness comes from OS process evidence, and missing evidence stays uncertain.
Only live or unresolved execution, a live Process or an unresolved provider turn,
holds Task completion, cleanup and landing. The caller inspects that history
before launching fresh work, which starts another Flow.

Independent helpers may carry the same Task and independent Git/PR
operations. Binding to done Work assigns a conversation without reopening it.
Legacy review boundaries remain readable history when their Work is terminal.

Chapter rotation preserves Task, AgentSession, PR and worktree
identity when moving started unfinished Tasks. Linear status changes converge
through fresh provider reads; there is no atomic repository-wide Chapter switch.

## Durable communication

Task steering posts Linear comments. Task Runs refresh comments into local
delivery events and starting context; publication, seed inclusion, and provider
acceptance are distinct evidence.
Steering an idle Task starts nothing. Wave guidance travels as extra
instructions to `wave/operate`.

```bash
lf comment INF-123 "keep the public name"
```

### Questions and sessions

These examples use current command spellings; the lifecycle contract is above.

```bash
lf -b implement
lf session list --interactive false --task INF-123 --json
lf session connect SESSION
lf session rename SESSION "Migration review"
lf session bind SESSION --task INF-123
```

Every conversation has one AgentSession regardless of launch surface. Connect
uses the existing engine where possible; client replacement retains its active
turn. Name, feedback, native identity and history survive reconnect and recovery.
A suggested title cannot overwrite a human-assigned title. CLI and Desktop use
the same action and availability reason.

Ready saves feedback and keeps the conversation open. Complete persists its
closed state and exact feedback before teardown. A Flow review returns
feedback for the following decision; it never selects that decision's edge.
Pane close, provider exit and readiness do not complete the review.

The desktop terminal pool keys on stable AgentSession identity. Rename, bind,
reconnect and replacement retain the pane and draft when reusing that surface.
Recovery after engine exit does not claim preservation of text never submitted
to Loopflow without separate UI evidence.

## Flow execution

```bash
lf task run INF-123                  # Project's Flow
lf task run INF-124 incident  # explicit override
lf flow example                     # with or without Task attribution
```

The driver compiles the graph, every router and Xor alternative, and holds it
with the cursor and return counts in memory. Definition changes affect new
Flows. A past Flow keeps the graph its driver recorded at launch. Backward edges stay finite graph structure; future loop passes are
not preallocated. Node IDs are local typed identities, not path strings.

Taking an Iterate edge updates the driver's cursor and return counters. The
node key and per-edge iterations on each FlowProcess step row identify the pass,
including overlapping backward edges. Neither loop passes nor authored subflows
create Flows or lifecycles.

Task and taskless execution use one driver and need no synthetic
planning records. The draft migration archives each earlier Flow row's name,
state and last cursor as a `legacy_flow` observation on the Sessions it opened,
then drops the Flow tables and the Session's Flow column. Configured acceptance
and the machine's current-state cutover remain separate from schema verification.

A decision returns Advance, Iterate or Blocked in the final answer of the turn
its step Process captured. Blocked records its reason and stops at the current
position. Only Advance and Iterate move the cursor. Failure or interruption
cannot submit a verdict. There is no file-backed cursor. Recovery of an
uncertain mechanical effect still requires inspection; cursor movement alone
cannot establish exactly-once external effects.

Finite Wave planning uses ordinary attributed agent conversations. Historical
Wave continuations remain preservation evidence and confer no new execution or
process-control authority.

## Task delivery algorithm

A Task binds planning to one active PR branch and managed Git worktree. A
checkout on that branch identifies the Task regardless of its upstream; the
stored path supplies placement for explicit Task selection. The current delivery
implementation can rotate a settled Task onto a later serial branch. Once that happens, the old branch no
longer identifies the Task. Collapsing the Task lifetime to one Linear-associated
branch remains a separate delivery simplification.

```text
Linear Issue
    |
    v
Task row ----> managed worktree ----> commits
    |                                  |
    |                                  v
    +-----------------------------> GitHub PR
                                       |
                              checks / repair / merge
                                       |
                              complete Task
```

1. `lf checkout` resolves one Linear Issue inside one Project and creates
   or reuses Task Work, its worktree, and its serial PR identity. It starts no
   agent work.
2. Independent `lf --task ...` conversations may work in that substrate directly.
   `lf task run` places the Task, defaults its Flow, then runs it through the
   same `lf run` path as any other launch.
3. `lf commit` snapshots the worktree. `lf pr publish` creates or refreshes the
   current PR without opening a browser.
4. `lf submit` leaves the exact-head merge click to a person. `lf arm`
   and `lf land` request exact-head auto-merge, record the landing, and
   return; `lf pr reconcile` checks recorded landings once. All operate on Task delivery state when it exists and require
   no Flow-driving claim or execution receipt.
5. PR landing is fenced by landing generation. A check repairs an unchanged
   incident once; a moved head requires fresh evidence. A blocked landing still
   observes merge, and explicit arm or land resumes it.
6. Verified merge completes the Task unless explicit remaining work keeps it open. Serial PR
   rotation and separately stacked dependent Tasks retain their own identities.

GitHub remains merge truth. SQLite stores the observed PR/head/check/disposition
needed to resume safely; it cannot declare an unmerged PR merged.

## OS locks and allowed contention

Loopflow uses advisory OS file locks for exact local critical sections. The
open file descriptor holds authority; the JSON file is a readable receipt.
Process death releases the kernel lock even if the receipt remains, so the next
operation can clean or explicitly adopt stale metadata.

### Git mutation and sync locks

For a Git worktree, `absolute_git_dir` selects the real Git directory, including
the linked-worktree case. Sync coordination lives beneath it:

```text
<absolute-git-dir>/loopflow/rebase-owner.json
```

Agent conversations receive no worktree writer token. Commit, PR mutation, restart
checkpointing, and land take short OS-held locks only around their exact Git
mutation. Independent agents may edit and run concurrently; the shared
worktree remains the durable blackboard.

A sync takes an exclusive lock on `rebase-owner.json` for the complete Git
sequencer lifetime. New agent launches refuse while that sync lock is live.
The exact `LF_GIT_OPERATION_ID` lets only the operation's recovery child
continue or abort inside the fence. A raw or crashed sync can be adopted only
through the explicit adoption path, which mints a new id.

Unowned and stopped Git operations do not block agent launch. A stopped operation's
receipt survives so continuation can adopt its pinned target. Raw merges use
`lf sync --continue --adopt`; launching an agent grants no operation ownership.

Therefore:

- agent + agent is allowed;
- reader/build/test + agent is allowed;
- live sync + new independent agent launch is blocked;
- a running agent does not reserve the worktree against sync;
- sync + its exact recovery child is allowed;
- stale JSON with no OS lock is not a live owner;
- raw Git outside Loopflow is not compelled by these advisory locks.

### Short mutation and machine locks

`<absolute-git-dir>/lf-pr-mutation.lock` serializes only Task PR/head mutation
sections; a second such operation fails fast while the first guard is alive.
Wave locator locks serialize relocation filesystem ownership. A
`<store>.migration.lock` serializes backup plus schema application. The current
promotion operation holds `$HOME/.lf/promotion.lock` exclusively for its full
upgrade transaction. These locks do not turn a conversation ID into authority.

## Process ownership and control

Causal ancestry and conversation identity are not process ownership. Local child
handles permit the spawning process to control its child. Cross-process control
requires exact PID/start identity and the appropriate native scope, claim and
provider generation. Revalidate that evidence before every signal.

A driver can die while its engine continues. A saved endpoint alone is not
liveness; a missing endpoint alone is not engine death. Recovery reads surviving
native history and preserves unknown command outcomes. Client replacement leaves
the engine alive; Flow retry replaces an engine only after confirmed exit.
Neither operation authorizes terminating a shared engine to recover one thread.

## Machines and process topology

```text
Loopflow.app / shell / external harness
                 |
                 v
                lf ---------------- Linear / GitHub / provider auth
                 |
       planning SQLite + repository/Git
                 |
                 v
          Task CLI ----- exact invocation claim
                                              |
                                              v
                                      provider harness
                                              |
                                              v
                                      AgentSession native history
```

Wave operations are finite conversations. Tasks drive their selected invocation; finite
`lf pr reconcile` checks observe delivery. Crossing Machines is an explicit `lf ssh` hop
whose target proves its Machine identity.

### Multi-Machine placement and execution

`MachineId` is stable identity; its SSH route is replaceable. `Placement` maps
Work to a Machine and stores eligibility, never liveness or signal authority.

```bash
lf observe <machine-id> ssh://jack@mini.local
lf wave place <wave-id> <machine-id>
lf ssh <machine-id> --wave product wave/operate
```

The target uses its own store, repository, provider homes, OS locks and payload
directory. Reads remain Machine-local. Foreground SSH may explicitly forward
selected account authority; detached processes use installed credentials.

Wave selection always resolves `(canonical repository, slug)` to one UUID.
Bare-slug diagnostics fail when more than one repository owns the slug; no
read or mutation chooses one by order. A scoped lookup repairs an equivalent
legacy path spelling to the canonical repository in one transaction.
`lf wave rename <uuid>` is the only semantic locator mutation: it fences
the Wave chord, moves authored files, commits the new locator
transactionally, and leaves PM, Work, and Machine-placement rows joined to the
unchanged UUID. A target-local `.lf/tmp/wave-relocations/<uuid>.json` receipt
bridges the filesystem/SQLite commit boundary; retrying after a committed crash
finishes verified source cleanup, then removes the receipt. Repository moves
also require compatible configured PM Teams so relocation cannot impersonate
the separate `lf repo reteam` operation.

## Promotion and long-running old processes

1. Verify and install immutable versioned artifacts.
2. Copy the selected planning store, apply the candidate schema to that
   isolated copy, and prove the candidate can read it.
3. Atomically repoint the launcher used by future top-level processes.
4. Let already-running processes continue with their selected executable.
5. Restart only the Machine services and app surfaces actually being replaced.
6. Recover or roll back from the persisted artifact-selection receipt.
7. Garbage-collect old artifacts separately from activation.

The installation promotion lock serializes artifact selection and service
replacement. It does not discover, drain, stop, or settle conversations and it is not
held by ordinary harnesses. Store cloning remains useful because preview can
prove a candidate schema without mutating the selected store.

An already-running `lf` retains its executable and selected store path.
An execution-schema cutover must account for those writers before activation. On the first published-to-development switch, the process may keep
writing successfully to the prior production store after new commands select
the cloned development store; those writes become invisible to the new
selection. A later development-to-development promotion may reuse and migrate
the selected store, so an old writer may instead fail against changed schema.
Promotion pauses and replaces the known services it owns but does not discover
every shell or provider process. The clone proves candidate readability; it
does not provide cross-store write continuity or old-schema compatibility.

## Truth and projections

The map is the ownership index. Truth remains distributed across Machine-local
SQLite, repository files and Git, the command journal, Linear, GitHub, and provider
homes or Doppler; none is a fallback authority for another.

Intentional copies stay read projections:

<!-- architecture-projections:start -->
| Projection | Authority copied | Freshness and consumer |
| --- | --- | --- |
| [`PmSnapshotRow`](../rust/loopflow/src/store/mod.rs) / `pm_projects`, `pm_items`, `pm_wave_projects`, `pm_wave_sync`, `pm_issue_changes` | Linear planning | Shared entity ingestion orders provider revisions and retains last-good facts when refresh fails. Task detail and Wave views join the same facts; sync age and invalidation remain explicit. |
| [`TaskLinearObservation`](../rust/loopflow/src/work/task/mod.rs) / `task_linear_observations` | Linear Issue state | Reconciliation records provider evidence before applying lifecycle changes. |
| [`GithubObservation`](../rust/loopflow/src/work/task/mod.rs) / `task_prs`, `ci_incidents` | GitHub PR/check state | Webhook or foreground reads update Task delivery evidence; GitHub remains merge truth. |
| `tests/fixtures/dto/` | Rust `lf --json` DTOs | Rust and Swift fixture tests reject required-field or enum drift. |
| `tests/fixtures/migrations/` | Ordinal-free migration drafts and the Python canonicalizer | Rust build/runtime and Python release tests reject ordering, body-byte, checksum, and graph-error drift. |
<!-- architecture-projections:end -->

`lf wave status` and `lf roadmap` derive Task conditions from Work, Process and
PR facts. AgentSession and Process own ancestry; immutable history keeps
earlier attribution. The transitional joins are listed in the cutover status. No projection acquires launch, Work-mutation,
credential or signal authority. UI grouping is cached presentation, not another
attribution store.

## Extension rules

| Area | Safe extension | Architectural constraint |
| --- | --- | --- |
| Execution queries | Add an index for a measured Process or AgentSession query | Filter before payload IO; identity/ancestry come from the owning row, never fallback files |
| Multi-Machine views | Fan out read-only commands through `lf ssh` | Do not centralize Machine-local execution ownership or silently mix local and remote scope |
| Process control | Publish birth-validated ownership at the launcher spawn seam | No PID/tmux/Work/telemetry inference |
| Planning input | Add a naturally keyed fact or provider observation | Do not create a global input revision protocol |
| Provider support | Add a provider adapter, account route, and normalized stream mapping | Provider credentials/finality remain provider-authored |
| Promotion | Add artifact roles or service adapters within the locked switch transaction | Artifact activation does not depend on conversation discovery |

A new writer must have one authoritative model. Avoid backend dispatch between
old and new representations, dual authoritative SQLite/filesystem records, mandatory
collector daemons, or planning capabilities derived from observation data.

## Appendix: compatibility seams

Compatibility survives only when it crosses immutable external history. Each
seam names its translation and deletion boundary; none is a second current
model.

<!-- architecture-shims:start -->
| Seam | Current concept | Source and removal boundary |
| --- | --- | --- |
| `shim:rams-alias` | Installed `rams/rams` command resolves to the Skill model. | [`SkillSource`](../rust/loopflow/src/lf/discovery.rs); remove when the external single-file command is no longer supported. |
| `shim:retired-app-replacement` | Promotion removes the previously shipped app bundle after the current app commits. | [`AppPromotion`](../rust/loopflow/src/lf/commands/install.rs); remove after the retired bundle name is outside supported installs. |
<!-- architecture-shims:end -->

## Appendix: historical-only vocabulary

The scanner matches exact phrases, not overloaded words. Provider resume
sessions, tmux sessions, and `session.launch` are current. The authored chat
reference `project:<slug>` is also current; it is not the old Linear-label PM
model.

<!-- architecture-vocabulary:start -->
| Retired term | Allowed scopes | Current language |
| --- | --- | --- |
| `Project Session`, `project_sessions`, `task_sessions` | `rust/loopflow/src/store/migrations/`, `rust/loopflow/src/store/migrations.rs`, `rust/loopflow/src/store/tests/fixtures/`, `release/` | Stable Project/Task **Work**; AgentSession owns the conversation and native history; a Flow is a driver Process and its step processes. |
| `session context`, `LF_SESSION` | — | Typed Work ancestry and Process/AgentSession provenance; transitional launch environment names are listed in cutover status. |
| `lf radio`, `agent bus` | `release/` | Typed Work observations, Steer, synchronous questions, and review FlowSteps. |
| `pm.linear_project`, `projects/<slug>.md` | `release/`, `rust/loopflow/src/ops/project.rs` | `pm.linear_initiative`; Linear Initiative → Project → Issue. The Project operation reads the historical YAML field once into SQLite and retains original bytes; later file edits are inert. |
| `machine-local host`, `machine-global command`, `machine-global mutation`, `machine-global reservation` | — | Machine-local keeper, command, mutation, or reservation. |
<!-- architecture-vocabulary:end -->

The following map gives the destination of retired or transitional representations.
Cutover status identifies which runtime consumers remain; the map does not claim
their deletion is finished.

| Historical or transitional representation | Target owner |
| --- | --- |
| Wave-scoped Chapter / `wave_chapters` | Linear Project status; Chapter is the shared In Progress name, with no stored object |
| Recommended Flow / `flows.recommended` | Project's Workflow definition selection |
| `FlowPosition`, `PinnedTaskFlow`, `task_flow_positions` | The Flow driver's in-memory cursor; FlowProcess records the graph and each step's node |
| `FlowRun`, `flows/<id>/position.json` | The Flow's driver Process and step processes |
| Subject selector list on a Run | Typed AgentSession/Process ancestry and immutable event attribution |
| Four Session projections, Ask files, composite boundary Session IDs | `agent_sessions`, keyed by stable Session ID, with driver Process and native history |
| Session name/resolution and provider attachment sidecars | AgentSession attributes, native identity and exact process evidence |
| Step occurrence / path-string node key | Driver Process ID, local node key and iteration tuple on the FlowProcess step row |

Canonical migrations, migration fixtures, and release notes retain historical
names because changing shipped evidence would rewrite history. Operational docs
and current runtime source do not. Chapter archives under `.lf/chapters/` are
dated evidence, excluded from live vocabulary and compatibility-seam discovery.

## Authority and failure invariants

- Linear owns Project status and shared planning. A Chapter is the shared name of
  one In Progress Project per Wave; partial rotation is visible and retryable.
- Wave instruments and observations survive Chapter changes. Missing target
  planning is unknown, not proof that an instrument is untargeted; it cannot
  erase an observed reading.
- An agent launch needs no planning parent, but requires writable admission and
  immutable captured input before provider side effects.
- Process is an actual lf process. AgentSession history owns native outcomes and
  usage; a Flow is a driver Process and its step processes, whose exits are its results.
- Command outcome, native completion, current liveness and Work disposition are
  separate facts. Missing terminal evidence stays unknown.
- Causal parentage and Task attribution confer neither process control nor Flow
  settlement. Exact process identity and native scope govern signaling.
- An authoritative conversation driver is singular; provider generation differs
  from driver generation. Passive readers acquire neither claim.
- A Flow consumes its selected successful native completion under version/claim
  fencing. Failed turns cannot donate verdicts or routes to successful retries.
- Complete closes a conversation or returns review feedback. Readiness and provider
  exit never choose an edge; a following decision owns its own navigation.
- Bind is write-once, same-target idempotent and valid for done Tasks. CLI states
  the permanent target and writes; Desktop confirms. Historical attribution and
  set-once Started survive assignment and driver changes.
- OS locks cover their named critical sections. Sync admits only its exact
  recovery child; causal identity does not bypass Git or PR mutation authority.
- Store copies preserve evidence without acquiring process authority. Promotion
  and historical-writer continuity remain separate proof obligations.
- Reads are Machine-local unless explicitly routed by `lf ssh`. Indexed summaries
  precede payload IO; retired files are never identity fallback.
- DTO fields are required unless explicitly optional; Rust/Swift consumers and
  fixtures migrate together.

## Drift proof

```bash
uv run python scripts/check_architecture.py
```

The bounded check materializes the live schema (including drafts), discovers
root CLI families, binaries/internal process commands, any local HTTP routes,
provider kinds, literal Rust subprocess edges, read projections, declared
shims, and exact stale vocabulary. Every discovered item must occur exactly
once in the map or its named inventory. It validates the map's source links and
reports mapped/discovered counts. The vocabulary scan covers active top-level
docs, product docs, prompts, scripts, website code, production Python/Rust/Swift
trees, migration SQL, and release history. Generated `website/docs/` is excluded
because the authoritative `docs/` source is already scanned. Historical
allowances must shelter at least one current match, so dead scopes fail instead
of becoming a permanent allowlist; declared compatibility seams must retain
their exact source marker. The check does not pretend to interpret every Rust
type or sentence.

CI runs the same command for every proposed merge. The weekly Architecture
Drift workflow retains the JSON result as time-based evidence. A new owner,
projection, shim, or API either maps to an existing concept or updates this page
in the same change.

A Task Session is an AgentSession associated with a Task checkout. A Task can have
any number; repo and Wave Sessions retain their explicit scopes. This term does
not restore the retired Task Session controller or a second conversation store.
`session list --orphan` selects Sessions without Task association; it cannot opt
a Session out of checkout membership.

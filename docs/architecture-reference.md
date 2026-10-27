---
layout: default
title: Architecture Reference
---

# Architecture Reference

## Intent flows down, execution facts flow up

Intent means Tasks, Waves, Projects, Workflow moves, ownership, pools and
budgets. It has one owner of record: the planning provider, or Loopflow's
journal when the plan is local. Execution facts are Processes, Sessions,
GraphProcess rows and checkouts. The Machine that ran them owns them. Upward
copies (worker → controller → laptop) are read-only and keep their origin;
they become the final record only when the origin is gone. Upward copies are
planned, not built. The controller aggregates execution and is not the root.
There are no daemons or listeners; `lf --machine` over SSH is the only
transport between Machines.

This page owns the execution contract, its cutover status and the checked
current-source inventory. [Architecture](architecture.md) introduces the model;
[Execution](architecture/execution.md) and [Data](architecture/data.md) develop it.

## Cutover status

**AgentProcess cutover is incomplete (LOO-443).** Provider identity and attachment
now live on tagged process rows; recorded-agent views and scheduled settlement
share that inventory. Captured Claude/native replacement advances the capture's
snapshot without refreshing stale operations. Owned native launches record before
exec while preserving terminal groups; runtime verification remains open. Optional
admission types and foreground orphan cleanup still need conversion.
Headless launch refuses a missing attachment at runtime.
Invocation-owned account failover now settles the old process and preserves its
history before replacement. Top and Task gates share unfinished-row selection
and identity judgment; unknown agents and receipt-less lf rows remain visible.
Sampling failure retains Unknown rows, not an empty inventory. Public Task-status
and scheduled-entry agreement remain unproved. Native launch retains its frozen
provider attachment under a detached terminal
owner. Replacing an outer viewer changes only its input token. Whole-path stand-ins
cover creator exit, zero viewers and the original pending Codex request/channel;
on October 17, 2026, Jack Heart also confirmed that public Session attachment
restored an unanswered native Claude question after closing its terminal. Answering
it completed the original interaction with the same Claude PID and AgentProcess.
That demo used the branch binary and a disposable Machine home with the installed
provider account; installed-store integration and SSH remain unproved.
The LfProcess/LfSession rename is integrated from #1516. The
[execution guide](architecture/execution.md#record-actual-processes) records this
boundary; record fixtures do not establish installed acceptance.

**Implemented model; installed acceptance remains open.** LfProcess is one actual lf process;
LfSession is a Loopflow-owned continuable interactive or headless conversation; a Graph is
one lf process and the step processes it starts. Run has no
separate product lifetime. The SQLite owners, shared Graph runner, CLI readers and
coordinated Rust/Swift wire types implement this contract.

The implementation has `processes` and `agent_sessions`, with
captured input and provider outcomes beneath their Session owners. Jack Heart's
2026-09-30 compression decision removes historical import, intermediate draft
schemas and old format readers. The cutover retains current Work and links,
accounts/routes and resumable conversations. Configured acceptance is separate.

Current CLI examples use supported spellings. `lf connect` selects a Session ID,
Task or Wave. Retained native terminals attach without replacing their provider.
Explicit native resume of a structured Run may restart the provider while preserving
Session/history, without resubmitting input. `session attach` never restarts.
A Graph whose process died leaves its
Processes as history; nothing resumes it. `session list` supports
`--interactive false`, `--history`, `--task` and `--search`; `--all` means all
repositories. `--page --json` uses stable ID pages; Desktop retains earlier
observations until enumeration succeeds. `lf graph list/show --processes` reads
Graphs from their Graph process and step processes. Historical `lf history show` and `lf history replay` remain command spellings.
History exposes Session event and provider evidence; `RunSnapshot` and Session
`run_id` are removed across Rust, Swift and fixtures. Retaining these names in a command or
wire reference does not make Run a target owner. Coordinate migration with
external consumers before changing those references.

### Implementation and acceptance evidence

| Contract | Current evidence and next dependency |
| --- | --- |
| One Process per actual lf process | Ordinary commands record process ancestry and outcome. Help, rejected arguments and installation entry use an existing compatible ledger without initialization; unavailable storage remains an explicit gap. Public discovery and Rust/Swift wire fixtures cover the current shape; installed startup acceptance remains open. |
| Stable LfSession with separate attachment and AgentProcess | Admission/publication and Session readers use LfSession directly; ordinary automatic retry retains both outcomes with zero Run rows in the native fixture. Public Codex connect/client replacement preserves the active turn and sibling in the synthetic-Responses proof. Complete configured account/native Machine, stale-client exclusion and shared-AgentProcess preservation across every provider path. |
| Graph reads each step's result from its Process | A step's result is its process exit; a deciding or routing step answers through the Session turn its Process captured. Structured native results close the earlier Codex decision-retry transport failure in the real-Codex/synthetic-Responses fixture; configured-provider acceptance remains unproven. OpenCode batch and native terminal launches share an authenticated private server and API-confirmed identity. Ordered native messages attribute assistant steps to admitted initiating inputs across steers; an idle marker plus a successful nonempty answer settles that input. Interrupted execution is never automatically continued. The separate Task provider launcher and retired Run owners are deleted; configured recovery across providers remains open. |
| One started Graph is one lf process and its step processes | Jack Heart's 2026-10-05 decision: a Graph adds nothing on top of running `lf graph ...` in a Task worktree, and its record is the lf binary's ordinary logging. Jack's later 2026-10-05 statements put that logging in the Graph process: an append-only GraphProcess row with the Graph's name and compiled graph, and one row per step it starts. The Graph process holds one cursor and its return counters in memory; steps are plain commands that know nothing of their Graph. Configured acceptance remains open. |
| Complete recovery | Six standalone native fixtures use real Codex with synthetic Responses/private Machines, including attachment/AgentProcess loss and automatic retry. Managed dispatch has synthetic successor-history proof; configured managed provider/account continuity remains unproven. |
| Current-state cutover | Three direct migration groups retain current database state. Release conversion must also recover resumable native identities from a frozen filesystem snapshot. Finished history and intermediate branch schemas are discarded. |
| Indexed discovery and usage | Runs/usage/telemetry/activity and landing conclusions select LfSession input history before payload decoding, preserving per-input attribution and windows. Native receipts without a captured input/start remain discoverable with unknown ownership and partial usage coverage. Debug measurements at `6d76f74926` used 20,000 Sessions, 5,000 Graphs and 100,000 Processes: bounded warm CLI reads were 291–313 ms against a 300 ms empty-store baseline. The broad Process search miss was 362 ms, with a separate 64 ms SQL control. These are uncontrolled-cache local measurements, not release or Desktop latency. History uses captured event sequences and exact native references. |
| Desktop and wire agreement | Off-roadmap ancestry and pane/draft retention pass unit and mounted native-terminal fixtures. Graph and membership wire use captured numeric IDs in Rust/Swift. History and usage use the coordinated Session wire types. Configured Desktop continuity remains unproven. |
| Status-owned Chapters | Focused fixtures cover rotation/workflows, populated current-state adoption, partial/competing-plan preservation and second-Machine convergence. Configured Linear rotation remains unproven. No Chapter table or packet belongs in the model. |
| Integrated acceptance | Affected checks, configured provider/Desktop, backed-up current-state conversion and final consistency remain required. Branch fixture passes do not establish installed acceptance or promotion readiness. |

Admission preserves saved executable/Machine/database handoff and failed-command
diagnostics. Process owns command outcome and error; Session events own provider
outcomes. Exact PID/start receipts establish process identity. Passive readers
acquire no process or Graph authority. Configured-provider and Desktop acceptance
remain open; fixture results establish only their exercised boundaries.

## The system grows outward from a direct Skill launch

The direct Skill launch is the kernel of Loopflow. It remains useful with
no Wave, Project, Task, or daemon. Agent admission requires its Machine's writable store. Higher layers
supply composition, durable context, delivery, ownership, and views around its
discovery, prompt, provider, harness, and evidence components. Their
boundary executors remain domain-specific because their settlement rules differ.

```text
                        +--------------------------+
                        | UI and read projections  |
                        | status / roadmap / app   |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | Task ownership           |
                        | person + Machine         |
                        | lf --machine             |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | Task delivery            |
                        | worktree / PR / CI       |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | Task Graphs               |
                        | one lf process per Graph  |
                        +-------------+------------+
                                      |
                        +-------------v------------+
                        | tracked Work + Graphs     |
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
- **Graph composition:** sequence Skill and mechanical Command nodes, route Xor
  branches and follow backward edges. Every step is autonomous.
- **Durable planning:** preserve Wave, Project, and Task intent across crashes;
  each Task Graph is a fresh Graph process and the steps it starts.
- **Task delivery:** attach one active remote branch, worktree, and PR to concrete Work.
- **Task ownership:** a Task is owned by a person on a Machine, or unowned;
  commands that change it run on the writer Machine through `lf --machine`.
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
 Wave -> Linear Project -> Task          LfProcess / LfSession
       stable Work           indexed history + payloads
          |
          v
 repository + Git worktrees

exact local races use OS locks; remote execution uses lf --machine
```

- **Tracked Work** records purpose, input and convergence; each Graph process
  holds its position and advances it.
- **Execution evidence** keeps command outcomes, provider turns and Graph results distinct.
- **Delivery** coordinates worktrees, commits, PRs, CI, and merge.
- **Machine authority** places Work and scopes local credentials, processes,
  files, and locks to a Machine.

An identifier in one area is not authority in another. The detailed boundaries
below are the architecture's central constraint.

### Identifier spelling

Structured IDs have one stored and emitted spelling; input may use any casing.
Domain constructors, parsing and serde normalize (`id.rs`), and SQLite triggers
reject a noncanonical write with `canonical identity required: <table>`.

| Domain | Canonical form |
| --- | --- |
| Wave, Trace, LfProcess/Graph, steer, attachment and owned UUIDs; Linear Issue, Project, Initiative, Team, comment, attachment and assignee IDs | Lowercase, punctuation retained (simple, hyphenated, braced, URN) |
| Task, Project, repository, Machine, PR, landing, tool response, cron receipt | Lowercase prefix and UUID (`task_`, `proj_`, `repo_`, `home_`, `pr_`, `landing_`, `response_`, `cron_`) |
| LfSession (`LfSessionId`) | Lowercase `session_<hex>` or bare 32 hex; other historical names keep their bytes |
| Task label | Lowercase local `lf-<hex>`; uppercase Linear `<TEAM>-<n>` |

Opaque values keep their bytes: provider conversation and turn IDs
(`AgentSessionId`), account IDs, request and cursor tokens, credentials, hashes,
capture locators, names, slugs, paths and arbitrary payload content. A Task
selector resolves each domain separately (`TaskSelector`), so an opaque
external ID is never treated as a ticket; a Session selector tries the
canonical LfSession first and gives native lookup the original text.
Metric evidence keeps its originally hashed Wave spelling while its Work
association is canonical. Files named from an identity keep their retained
locators (below). The `canonical_structured_ids` migration converts released
rows and declared JSON paths in one transaction and aborts on any collision.

## Planning and execution vocabulary

These are the accepted product owners; see [cutover status](#cutover-status)
for the current implementation boundary.

```text
Wave --< Linear Project --< Task --< Graph (LfProcess)

LfProcess --< LfProcess             causal ancestry, not control authority
LfProcess --< AgentProcess          immutable launching parent
LfSession --> AgentProcess         nullable current provider; old rows survive
AgentProcess --> LfProcess          nullable current attachment, fenced by token
LfSession --< history              immutable provider outcomes and usage
Graph LfProcess --< step LfProcess   GraphProcess graph; result is the step's exit
```

| Concept | Owns |
| --- | --- |
| Repository | Wave membership and repository-wide Project rotation |
| Chapter | No stored object: the shared name of each Wave's In Progress Project |
| Wave | Enduring objective, memory, cadence, budget and metric instruments |
| Project | Linear status, Tasks, KRs and targets |
| Task | Worktree, optional PR and every associated Session, Graph and Process; no Graph is privileged |
| Graph | Reusable authored graph of agent/mechanical/router nodes; a running one is a Graph process and its step processes |
| LfProcess | One actual lf process: durable `id` (`LfProcessId`), optional Unix `pid`, immutable causal ancestry (`parent_lf_process_id`) and command completion |
| AgentProcess | The provider OS process Loopflow started: a `processes` row of kind `agent` with its parent LfProcess, served Session, attached LfProcess and fresh attachment token; survives attachment handoff |
| LfSession | Loopflow conversation identity, title, feedback, current AgentSession and provider history |
| AgentSession | Provider-owned conversation, identified by opaque AgentSessionId for resume, turn matching and account attribution; no separate table |
| Machine | Store, payloads, credentials and exact local process authority |
| Task ownership | (person, Machine) or unowned, read from the Task's append-only ownership log; no authority over merely observed processes |
| Steer publication | Immutable authored direction with independent Session delivery |

The LOO-441 names are integrated. The common `LfProcess` wire projection
still carries both lf and agent rows during the AgentProcess cutover.

## Definition and execution projections

A Workflow's definition is a graph: `WorkflowDefinition` is projected from a
`GraphDefinition`, its `interactive:` steps as stages and the slices between them
as routes. `WorkflowCatalogEntry` carries that projection, while
`GraphCatalogEntry` carries the compiled graph and `GraphComposition`
disclosure tree. Invalid sources remain visible with their error.

`lf wave workflow list/source` reads graphs; `show/set`
reads or changes the Wave's default, independently of Project history. `set --file` stores an edited graph in the Wave.
Task `workflow show/restart` handles its captured instance. Restart is the existing
move to `start`, preserving graph, moves and execution history.

Task status and roadmap separate `workflow_name`, `latest_graph_process`,
`execution` and `run_control`. The latest `GraphProcessDetail` is ordinary
execution evidence, including completed graphs, not a second Task lifecycle.
Task work and watch carry `graph_processes`. Session membership references the
exact `graph_lf_process_id`; Workflow moves retain their Task-run `lf_process_id`.
Persisted graphs and historical capture formats remain unchanged.

## Core models and APIs

Constructors and mutation APIs validate within the writing transaction. DTOs
carry typed ancestry; CLI names and issue identifiers resolve at the boundary.
There is no replacement Request, Execution, SkillInvocation or generic attempt
object. History entries have stable references, not independent lifecycles.
Jack Heart selected AgentProcess on October 9 for the actual provider OS process,
not for an attempt. LOO-443 records it in the shared process table; unconditional
launch recording is typed only at runtime. AgentProcess identity replaces the
provider generation in caller provenance, turn origin and Program Status.

| Owner | Authoritative fields and operations |
| --- | --- |
| `processes` | Durable `id`, lf/agent kind, nullable Unix PID/birth, immutable parent, command, cwd and terminal evidence. Agent rows own served Session, provider/launch mode, endpoint, attachment/token and exit reference. Lf rows retain incoming agent provenance. Admit, attach, finish, filter/page. |
| `agent_sessions` | Stable ID, purpose and independent interactive flag; title/provenance, request/feedback, typed Task/Wave ancestry, native identity and current AgentProcess reference; reserve, connect, bind and rename; historical feedback/completion retained |
| `tasks` | Project, issue, durable disposition and worktree/delivery facts |
| `task_ownership` | Append-only `claim`/`handoff`/`unclaim` entries (`own_…`) with person, Machine and time; migration-only `recorded` entries are never delivered |
| `projects` | Wave, stable Linear Project identity, status, shared chapter name, Graph and planning facts |
| `waves` | Stable identity and repository address; objective, memory and instruments are read from checkout files |

### Conversation and attachment lifetime

Every agent conversation is an LfSession: skills, inline prompts, helpers,
reviews, interactive and headless work. Default views select interactive
conversations. `lf session list --available` selects conversations Available for a
person, using the same Rust projection as Desktop. `session_activity` holds both
the attached LfProcess's latest stream reading and an optional terminal-reported snapshot.
Its optional preview carries bounded provider text, native thread/turn identity,
source and observation time. Only the current live AgentProcess and attachment
can project it. Provider selection changes clear it; a new Codex turn clears the
previous excerpt before output arrives. Desktop applies a 30-second working-text
lifetime without changing execution state; current Available results retain their
source age. Missing or disconnected evidence uses a labeled summary.
A recorded AgentProcess alone decides whether its Session is active: completed
means closed; otherwise exact PID/birth evidence is live, dead or unknown. Unknown
is never Available and never ages into replacement or checkout-cleanup authority.
Every owned launch records an AgentProcess before exec, including native terminals.
Sessions without one remain unknown; client files are not read. The Session's
`transport_lf_process_id` names its detached terminal owner. The
execution child owns capture settlement and the frozen provider attachment;
viewers receive separate revocable input tokens. Passive viewing takes no claim.
Saved sockets are offered only for live records; a live endpoint's connection
failure is an error, not permission to start another agent. Input completion uses
its latest turn's LfProcess completion (the capture owner before any turn), not
attachment exit events. Events retain
turn outcomes and history, not process liveness.

Availability requires positive live evidence before pagination.
The current AgentProcess's reports take precedence: any blocked record, or
idle for an interactive Session, means Available. Other reports and explicit clear
suppress inference. Without reports, an unanswered question, interactive hand-back
or 120 seconds of silence without an unresolved tool call means Available.
No current reading is unknown; replacement providers need fresh observations.

Desktop consumes the embedded Ghostty action for every retained surface. Shell
panes keep local records; Session panes publish bounded snapshots through
`lf session observe-status` with the current terminal receipt and the
reading's AgentProcess. Stream identity and increasing sequence fence replacements in SQLite.
An unchanged provider keeps reports across attachment handoff. Report text is literal
and display-sanitized; protocol IDs never select Sessions or grant process,
Graph or completion authority. Detached relay observation remains unimplemented. Explicit filters expose headless and
completed history; `--all`
continues to mean all repositories. Interactive mode grants neither review
completion nor Graph authority.

A primary Session is the one ongoing conversation of a scope. It is an ordinary
interactive conversation whose row names that scope; a repository and each
Wave have at most one uncompleted primary. The repository's needs no Wave,
Task or planning provider. `lf connect` finds or admits it, prepares its
workspace, reserves the first input and owns the foreground launch; `--prepare`
stops before launch. Task primaries are admitted lazily on the owner Machine, on first connect;
explicit Wave admission also admits the repository conversation. A Session may
have no current capture: metadata admission does not claim a Task, prepare
a prompt or start a provider. The first captured input, or binding a previously
captured conversation, starts work.
`resume` is an alias. `connect --fresh` retires the predecessor as primary and
admits the successor in one transaction, keeping the previous client and history.
Scope locks serialize admission and fresh requests. Each explicit fresh request
creates a new primary; ordinary connect recovers the current one. Primary grants no Graph, review or process authority, and
the Wave's goal and memory reach it as ordinary Wave context.

A Task's primary is one of its own conversations, named by
`tasks.primary_session_id`; its row carries no scope mark and it stays a Task
member. `lf connect --task ISSUE` keeps the chosen primary, else selects the
sole unfinished interactive conversation, else the most recently used by
native human input, then last opening. It creates one when the Task has none.
Opening an exact Session leaves the Task's primary unchanged.

AgentSessionId preserves the provider's exact id, including non-UUID ids. The
selected AgentSession belongs to the LfSession row; the provider endpoint belongs
to the AgentProcess. Captured provider observations retain native resume references.
A replacement preserves LfSession identity. Immutable turn/capture history keeps
earlier AgentSessions and their account attribution: replacement does not
reassign old usage. A new AgentProcess resumes the selected AgentSession.
No reader needs an independent provider-conversation table.

SQL columns and captured JSON retain `provider_thread` / `provider_session_id`;
Rust uses `agent_session` and Swift uses `agentSession` with the same string
encoding. This preserves existing native evidence and DTO fixtures without
a naming-only migration. Provider protocol names remain vendor-owned.

An LfSession can have many historical attached Processes and at most one current
attachment. Compare-and-set issues a fresh opaque token on every claim and release,
including A → B → A. The token has no independent lifecycle. A continuing
AgentProcess keeps its identity and origin through handoff and an unattached
interval. Old clients may retain provider history but cannot start/steer turns or
change current attachment, connection, process evidence or stream activity.
History retention grants no native-write or Graph authority. Passive connection
acquires no claim. Dispatch fences include queued native RPCs and approval replies, not only database claim updates.

Admission reserves an AgentProcess row; spawn request, exact OS identity and
observed exit update that record. Historical provider receipts remain immutable
history, not current lifecycle authority. Missing identity or exit stays unknown;
Lf completion never proves the AgentProcess exited. Saved native thread identity
is loaded before account selection.

Native dispatch and attachment claim, release, and exit share a Session-scoped OS
lock beside the canonical database path. Attachment validation releases the SQLite
mutex before provider I/O; history and other Sessions keep using the database.
Lock acquisition has an OS-clock deadline independent of the provider reactor.
Lock files retain their inode across process exit; deleting a live lock file
would let two processes own different locks for the same Session. The Session's
immutable `fence_locator` retains that address independently of its ID spelling.
Process rows likewise retain receipt basenames and trace journal directory names
in `receipt_locator` and `journal_trace_locator`; an identity conversion never
renames those files. New Processes in a retained trace reuse its directory name.

Identity migration tries affected Session fences without blocking and holds them
through SQLite commit or rollback. Busy dispatch, live affected owners and unknown
receipt evidence defer the upgrade without changing the ledger. An unreadable,
malformed, unsupported or mismatched affected receipt remains unknown until it is
repaired; elapsed time does not grant permission. Old waiting callers re-read
attachment authority under the same inode, and canonical Session storage guards
reject recreating an owner with its prior mixed-case spelling.

Connect transfers the attachment while retaining the live conversation. Client
replacement claims the attachment before stopping the exact old clients; it leaves
the AgentProcess and sibling conversations running. Connecting is the only way an
AgentProcess changes its attached LfProcess.
Session resume is admitted when the previously attached LfProcess has finished or
is provably dead, and refused while it may be alive. A resumed Session always
reserves a new AgentProcess and resumes the saved native thread on it; it never
adopts the old one. A finished LfProcess closed its AgentProcess as it exited. A
dead one may have left it running, so its replacement first ends the recorded
PID/start identity under the Session lock. A recorded process that is still running
and cannot be ended, such as one outside its own process group or a Codex
AgentProcess serving an unrelated conversation, refuses the resume and leaves the
Session unchanged.
PID/start identity and native endpoint are operational evidence; conversation
identity, causality and elapsed time grant no signal authority.

An exact attached LfProcess exit closes its Codex AgentProcess and records the
end under the same transaction as attachment transfer. A former attachment's exit
cannot stop a transferred AgentProcess. Shutdown verifies the recorded PID/start
and the whole process group; an AgentProcess serving other loaded conversations is
left running with a close error.
Closing clears the live endpoint and retains the native thread ID and history.
Native terminal providers own their own process exit. An observed
normal or interrupted exit retires an unbound, non-primary conversation only
when that LfProcess launched its AgentProcess. Task/Wave conversations, primary
Sessions, Asks and Graph reviews remain available. A stopped turn or missing
process never completes a conversation, and retirement never settles a Graph.
Completed Sessions retain their history but contribute no current rows or counts.
Terminal Tasks cannot acquire current review obligations;
independent unresolved Asks remain discoverable.

LfSession history records provider starts, successful/failed/interrupted
outcomes, retries, durations and usage, correlated with driving Process and native
turn/receipt. Repeated receipts are idempotent. Missing measurements differ from
zero; cumulative samples are not added together. Later continuation never rewrites
an earlier completion a Graph already read. Provider completion and Process
completion are distinct: subsequent command work may fail after the agent succeeds.

### Process lifetime and causality

One actual lf process gets one LfProcess, including nested direct and agent-issued
commands. In-process wrappers reuse it and cannot settle it early; each Graph
step is a child process with its own Process.
The incoming `via_agent` bit describes the caller, not whether the command later
launches an agent. Agent-issued children resolve stable Session/AgentProcess
provenance to the current attached LfProcess once, at admission. Delayed children
of a replaced provider retain their historical origin; existing parents never change.

Process outcomes are succeeded, failed or interrupted when observed. Unobserved
completion, exit code and signal stay unknown. The outer command owns terminal
settlement, including interruption. Bootstrap/installation logging must not open
or migrate an incompatible installed store before its authority preflight.
Help, parser errors and installation entry paths append process evidence
only to an existing compatible ledger, without creating a Machine, seeding branch
data or applying migrations. An attached ledger stays pinned through command
completion. Missing or incompatible storage leaves an explicit recording gap;
inspection and installation recovery still work. Agent admission requires its Process
and conversation rows before provider launch, even when earlier observation failed.

### Graph outcomes and authority

A Graph is one lf process, the `lf graph <graph>`, `lf <graph>` or
`lf task run ISSUE [GRAPH]` process, and the step processes it starts as child
processes. Its ID is the Graph process's. Task and taskless execution share the
Graph runner. A Task observes every LfSession, Graph and Process associated with its
checkout, plus explicitly bound work.
The shared Rust association reader includes checkout descendants at path component
boundaries and retained paths after removal. It neither follows causal ancestry
nor rewrites recorded work or usage. Session inventory exposes `task_ids` so
Desktop navigation uses the same membership as Task status.

No Graph is privileged: every one naming the Task, or run in its checkout,
is equally its work. `task status`'s `work` contains all Sessions, Graphs and
Processes, including headless and closed history. Its `execution` and Task Graph
record observe the most recently launched Graph only: `none`, `latest` or
`finished`. The one control is `start`, available whenever the Task can run,
even when an earlier Graph exists.

Cleanup, abandon, restore and landing repair treat all Task Graphs alike and ask
one question of one gate: does OS process evidence show any of the work's
unfinished Processes live or unknown? A stopped Graph is history and blocks
nothing. A Session's turns, reserved inputs and retirement are conversation
history and never block. A process is never blocked by its own caller lineage.
A Process that began before the machine’s last boot has exited; live or unknown
execution since then still blocks. Passive membership grants no control.
Template composition compiles into the graph, which the Graph process holds in memory
with its cursor. Loop passes are node/iteration positions and lenses over the
step processes. They have no separate lifecycle. A past Graph keeps the graph it
launched with, whatever its YAML says now.

Each step is the command a person would type, started as a child of the Graph process:

```text
lf --batch [options] skill <name> [message]       # agent step
lf <command> <args>                               # operation step
lf -b connect <session> --message <message>     # correction of an answer
```

Headless Session correction reserves the next captured input and claims its attachment
in one SQLite transaction after provider-custody checks. A rejected attachment claim
leaves the previous capture unchanged. Publication failure retains the reservation
and releases its attachment without reporting a provider turn.

A step is told nothing about its Graph and writes nothing about it. The Graph process
writes GraphProcess, and only by appending: `graph_processes` holds one row per Graph process
(Graph process, Graph name, graph as compiled at launch) and `graph_process_steps` one
row per step it started (child Process, graph node key, per-edge iterations). The
Graph process finds a step's Process as its own newest child. Running, finished and each
step's result are read from the Processes. Nothing reads the record back to resume,
and no command changes a running Graph; the control is ending its Graph process.

A step's result is how its process exited. A deciding or routing step gets its
answer contract in its message, and the Graph process reads the final answer of the
Session turn that step Process captured, following transport delegation
to the served command without including commands the agent ran. It accepts a JSON
value wrapped in prose or a code fence. An invalid answer is corrected by resuming the same
conversation: at most two corrective turns, then the Graph fails. After an
operation step the Graph process looks for a landing of its checkout that is still
being watched; the Graph stops there, and neither failed. Mechanical steps create
no LfSession.
A conversation attachment handoff grants no Graph authority.

A Graph is `current` while its Graph process has no recorded exit, `completed` when the
Graph process succeeded and `stopped` when it exited before the last step. A killed
Graph process leaves dead Processes as history. Nothing restarts or resumes it. The caller,
normally the Task conversation, inspects `lf graph show ID --processes --json` or
`lf task status`, then launches fresh work. Unknown liveness stays unknown.

### Attribution, binding and ownership

Task requires a repository and may have a Project; its Wave derives from that Project.
Session constructors use the Task’s current membership and reject explicit disagreements.
A Session links to its Graph through the Process that captured its input, whose
parent is the Graph process, and shares that Graph's nullable Task. `work_source`
is declared, checkout, inherited or bound when known. Historical unknown membership
stays Unknown; absence of evidence does not become Independent.

Bind is write-once: null to a Task, validating any explicit Wave against its membership. Same-target bind
is a no-op; reassignment and clearing are unavailable. Done/landed Tasks remain
valid without reopening them. CLI states the permanent target and writes the assignment; Desktop confirms
the exact target before writing. CLI adds no confirmation prompt. The writer compares the selected
conversation/attachment and ancestry atomically. Binding cannot alter Graph membership
or bind one member of a taskless Graph inconsistently with its owner.

Jack Heart's 2026-09-30 decision retains prospective usage
attribution: bind records assignment time; earlier usage retains its owner.
The history reader owns this single choice. `lf usage --binds` compares it with
post-hoc attribution from the same history. Preserve active-turn start/assignment
evidence; unknown allocation remains unknown rather than inventing a token split.
Project/Task moves preserve immutable historical attribution while
validating current ancestry.

`lf task run` and `lf task checkout` claim an unowned Task when they allocate its
checkout; that is actual work reservation. General command observation and bind
never claim. Completion, abandonment and reopening write no ownership entries, so a
Done Task shows its last owner as history. The migration recorded one `recorded`
entry, person unknown, for each Task that had Started, a worktree or an admission.
Chapter retirement also checks authored work, PRs and
Graphs: a Task with any Graph has begun. Absent execution evidence alone cannot prove untouched backlog.

### Readers and conversion

One indexed reader per object selects and pages identity, ancestry, command,
skill, title and status before opening payload files. No inventory scans manifests,
sidecars or live PRs to reconstruct identity. Bound conversations remain bound even
when absent from the visible roadmap. Desktop panes key on LfSession identity,
so bind, rename and attachment replacement retain the surface and draft.

Current-state conversion keeps resumable native identities and selected captures.
Finished conversations, old command events and unresolved historical SQL are
not imported. Runtime readers use only the final schema and current encodings.

### Chapters

Linear statuses are the owner: one In Progress Project per Wave, sharing a chapter
name across the repository. Planned Projects express future plans; Completed
Projects retain history. A Task without a captured Workflow selects an explicit request, its Wave default, then repository `workflow:`.
There is no Chapter row, packet or local switch. Rotation uses an explicit target
and stable Project identities, converges after partial mutations, preserves active
Task identity/worktree/PR/execution, and refuses unrelated competing plans.
A second Machine adopts the same state through ordinary synchronization.

Adjacent APIs keep their own authority: Task PR operations own Git/GitHub;
provider routing owns credentials; Task ownership owns routing; exact process
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
| Prompt assembly, Graph runner and provider processes | `rust/loopflow/src/prompt/`, `agent/`, `graph/`, `skills/`, `src/harness/` | 29,300 | Skill/Graph discovery, prompt assembly, provider subprocesses and streams |
| Tracked Work | `work/`, `pm/` | — | Wave/Task facts, Task delivery identity, planning/provider models |
| Storage and command journal | `store/`, `journal/` | 19,700 | SQLite access, migration runner, durable rows, outer command receipts |
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

Canonical Work readers in `store/sqlite/children.rs` serve Task status,
Project selection and Wave/Desktop lists. They read Task/Project
fields independently of Workflow position and provider inventory, retaining known
provider conflicts. Local state decisions and delivery receipts commit
together; observed Linear conflicts retire losing intentions while preserving
their receipts and local execution. Project ingestion stores
editable content so KRs and targets survive the shared reader. Task creation/edits
and Project creation/edits, rotation and membership moves use common transactions and field
receipts. Task deletion commits local removal and its pending receipt without changing execution.
Field conflicts go to the change sent last: an unsent save rebases onto the observed
provider value and still delivers, and a provider edit observed after an attempted
write is adopted.
Independent foreground loops deliver creation, fields, ordering, state, deletion
and comments; receipt-backed CLI/Desktop projections expose pending and uncertain
effects. The unconditional-update race remains a protocol limit.

<!-- architecture-map:start -->
| Concept | Truth and authority | Data structure | Persistence | Process owner | Public surface | External edge |
| --- | --- | --- | --- | --- | --- | --- |
| **User** — a person or external harness originating work | User-attributed actions author root input and decide effects that require user intervention. User is actor provenance, not a control credential. | [`Author`](../rust/loopflow/src/durable.rs) | Git supplies `user.name` unless personal Loopflow config overrides it; input records retain source author names. No User row; authored effects persist on the concept they change. | `lf` | `lf :`, `lf open`, `lf config`, `lf user` | `process:open`, `process:osascript`, `process:pbpaste`, `process:id`, `process:herdr`, `process:tmux`, `process:osacompile`, `process:plutil`, `process:/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister` |
| **Skill** — one reusable prompt with assembled context | Repository/builtin Skill Markdown is authoritative; discovery selects one source. | [`Skill`](../rust/loopflow/src/graph/mod.rs), [`SkillSource`](../rust/loopflow/src/skills/catalog.rs) | `.lf/skills/`, builtin Skill files, installed vendor Skill directories | `lf-prompt` | `lf skill`, `lf self sync-skills`, `lf list`, `lf help` (local command/definition discovery) | `process:python3` |
| **Graph** — definition and one execution | The Graph process compiles the GraphDefinition, including Xor paths, and holds the graph and cursor in memory. It records the graph and each step's node in GraphProcess; a step's result is its process exit. | `GraphDefinition`, `GraphComposition`, `GraphProcess`, typed node ID | `.lf/graphs/`; `graph_processes`, `graph_process_steps` beside the Graph process and step Process rows | One CLI Graph process per Graph; each step is its own child process running the plain command | `lf graph`, `lf task run` | — |
| **Workflow** — the graph a Task took up, its stages, and where the Task stands | A Workflow's definition is a graph: its `interactive:` steps are stages, where a person takes part in the Task conversation, and the routes between stages each run a slice of it. A Task's Workflow is the graph it took up, fixed from then on, and its stored position: at a stage, or on a route with the Process carrying it. Choosing a route and setting a stage are the two writes; the `lf task run` process carrying a route starts its slice as a child Graph, again when a Graph process fails, writes the arrival when one succeeds, and otherwise leaves the Task on its route. `lf ship` moves the Task to shipping and runs every remaining route, standing in at each stage; choosing a route into `end` also ships it. Every move is appended with its Process and note. It executes nothing itself. | [`WorkflowDefinition`](../rust/loopflow/src/workflow.rs), [`Workflow`](../rust/loopflow/src/ops/workflow.rs) | committed `.lf/graphs/` files and builtin graphs; a Wave records only the selected name; `task_workflows` (one row per Task), `task_workflow_moves` (append-only) | `lf task run` chooses a route, then starts its slice as a child Graph; `lf task workflow move` sets a stage; `lf ship` runs to `end` | Task status JSON (`execution.work.workflow`), the Workflow catalog, stage guidance in a bound Task launch | — |
| **Wave** — durable operating context with goal, memory, cadence, chat, and project selection | The Wave UUID is durable identity, carried in authored `GOAL.md` frontmatter. SQLite stores a one-segment name and optional parent Wave ID; the readable address is derived through parents within the canonical repository. Provisioning creates missing goal/memory files; Workflow graphs are repository files or builtins. Checkout files own goal and memory; SQLite owns identity and plan membership in both connection modes; Linear Initiative IDs are optional links. Renaming changes stored addresses, retaining execution; authored files remain in place. | [`Wave`](../rust/loopflow/src/work/wave/mod.rs), [`WaveLocator`](../rust/loopflow/src/work/wave/mod.rs), [`CanonicalRepo`](../rust/loopflow/src/repository.rs), [`WaveConfig`](../rust/loopflow/src/work/wave/config.rs) | `waves`; goal and memory are checkout files; historical relocation receipts remain retained | Finite Wave-attributed conversations; relocation commits expected addresses in one store transaction | `lf wave`, `lf wave list`, `lf wave status`, `lf wave show`, `lf wave workflow`, `lf cron`, `lf discord` | Discord when configured |
| **Chapter / Project** — shared current plan name and one selected Project per Wave | Project names, summaries and content save locally with per-field delivery/conflict receipts. Creation, selection, activation and rotation save locally with receipts consumed by foreground provider delivery. The repository chapter name is derived from its Waves' In Progress Projects. | `Project`, `ProjectStatus` | `projects`, `project_changes`, `project_events`; Linear Project observations | one local rotation transaction; foreground provider observation | `lf wave edit-plan`, `lf repo new-chapter`, `lf repo reteam` | Linear |
| **Live metric** — one reviewed measurement contract owned by exactly one Wave, plus revision-bound current evidence | `.lf/wave/<name>/metrics/*.md` owns meaning and Wave ownership; an accepted instrument observation owns its source-time fact; [`MetricPortfolioDto`](../rust/loopflow/src/work/wave/metrics.rs) is the sole derived reading shared across surfaces. Metrics inform KRs but never complete them. | [`MetricContract`](../rust/loopflow/src/work/wave/metrics.rs), [`MetricObservation`](../rust/loopflow/src/work/wave/metrics.rs), [`MetricPortfolioDto`](../rust/loopflow/src/work/wave/metrics.rs) | `.lf/wave/<name>/metrics/`, `metric_instruments`, `metric_observations` | Metric instruments write observations; foreground Rust readers derive bounded portfolios. | Status/roadmap JSON, Wave and Task prompts, the shared Swift DTO, and Mac Wave detail expose the same `metric_portfolio`. | — |
| **Task** — concrete repository work with optional Project membership | Local Task planning owns directive/status with optional Linear synchronization. The checked-out branch identifies the Task through its checkout placement; the stored worktree path is placement. Git upstream tracking does not select Task identity. Every Graph process for the Task is equally its work; helpers and delivery commands may mutate the worktree without driving a Graph. Git owns commits/branch state; GitHub owns PR/check/merge truth. | [`Task`](../rust/loopflow/src/work/task/mod.rs), [`TaskPr`](../rust/loopflow/src/work/task/mod.rs) | `tasks`, `relocated_task_processes` (observed checkout membership retained through moves), `task_creation_intents`, `task_changes`, `task_state_deliveries`, `task_issue_identities`, `task_deletions`, `task_events`, `task_prs`, `task_pr_repair_incidents`, `task_linear_observations`, `task_linear_ingested_comments`; Linear Issue; Git worktree | `lf task run` places the worktree, then drives a fresh Graph from its execution owner; foreground operations record delivery evidence | `lf task`, `lf pr`, `lf wt`, `lf sync`, `lf commit`, `lf scratch` | Linear |
| **PR landing** — one recorded intent to merge an exact PR head | GitHub is authoritative for the PR head, required checks, and merge. One landing generation admits one check at a time; an incident without a recorded response admits one repair. | [`PrLanding`](../rust/loopflow/src/pr_landing.rs), [`CiIncident`](../rust/loopflow/src/work/task/mod.rs) | `pr_landings`, `ci_incidents` | The process holding the claim: `lf pr reconcile` to observe and settle, the waiting landing (`lf land --wait-and-fix`, the ship process) to repair | `lf arm`, `lf land`, `lf pr reconcile`, `lf pr checks` | `provider:github`, model provider for `ci-fix`, `process:git`, `process:gh` |
| **Linear observations** — acquisition and ownership evidence | Repository/provider-scoped Project and issue observations retain provider evidence. Task and Wave readers use saved planning; ingestion preserves pending saves; the change sent last wins, so an unsent save rebases onto an observed provider edit and still delivers. Wave membership and sync observations reference those shared facts; change receipts invalidate admission without rewriting execution history. | [Linear acquisition](../rust/loopflow/src/linear/acceptance.rs) | `pm_projects`, `pm_items`, `pm_wave_projects`, `pm_wave_sync`, `pm_issue_changes`, `pm_project_name_cutover`, `project_binding_imports`, `project_transitions`, `project_transition_items`, `linear_team_checks` | On-demand team acquisition at planning/Graph/write boundaries and native turn-end hooks (`lf __context-block --moment stop`); watchers alone retain a slow clock. SQLite shares attempt freshness and the successful watermark; a Home-wide cooldown retains the reset deadline | `lf repo`, `lf refresh`, `lf task status` | `provider:linear` |
| **Asana and Notion planning** — the other remote planning providers | A repository names one team in `planning.team`. Asana keeps Tasks in the team project named after the repository and Wave Projects as team projects; Notion's team page holds linked Tasks and Projects databases. Both return the shared Linear observation types through one `PlanningClient`, so acceptance, receipts and steering stay on the Linear path. | [Asana](../rust/loopflow/src/asana/mod.rs), [Notion](../rust/loopflow/src/notion/mod.rs), [planning client](../rust/loopflow/src/ops/planning_client.rs) | Shares the Linear observation and receipt tables | Same planning/Graph/write boundaries as Linear | `lf account connect asana`, `lf account connect notion` | `provider:asana`, `provider:notion` |
| **Planning exchange** — shared Work changes across Machines | Canonical Work owns planning; repository configuration selects Git or Linear. Peer changes retain causal history and conflicts without importing execution or delivery authority. | [`PlanningSnapshot`](../rust/loopflow/src/planning_exchange.rs), [`PlanningGit`](../rust/loopflow/src/planning_git.rs), [`PlanningConfig`](../rust/loopflow/src/config.rs) | `repository_plans`, `planning_destinations`, `planning_peer_routes`, `planning_peer_changes`, `planning_peer_conflicts`, `planning_peer_imports`, `planning_peer_observed`, `planning_user` | The shared planning writer saves/imports; foreground exchange observes and publishes configured refs. | `lf planning`, ordinary Work reads and saves | Git refs |
| **Steer** — message guiding work | Immutable source/revision UUID; existing associations select Sessions | [`Steer`](../rust/loopflow/src/steer.rs), [`SessionSteerStatus`](../rust/loopflow/src/steer.rs) | `steers`, `session_steers`, `steer_batches`, `steer_deliveries`; `steer_adapters` binds provider callbacks. `task_comments` and `task_comment_deliveries` retain the separate planning thread and Linear sync | Foreground Task connections acquire comments independently of pending writes | `lf comment`, Linear issue comments | SQLite |
| **Tool response** — one idempotent response to a Work-scoped tool request | Stable Work identity plus request id names the response slot; a second, different answer is rejected. | [`ToolResponseWrite`](../rust/loopflow/src/durable.rs), [`ToolResponseReceipt`](../rust/loopflow/src/durable.rs) | `tool_responses` | Store transaction | Internal Work store API | — |
| **LfSession** — one conversation | Session row owns name, ancestry, readiness, completion and publication. Its current capture references an immutable history event written at reservation. Earlier events retain caller and Work attribution. Complete returns saved feedback; the following typed decision chooses navigation. | `SessionRecord`, `SessionId` | `agent_sessions`, `session_events`; `session_activity` holds attachment stream readings and terminal-reported status | Native turn observation retains start/usage/completion; Codex protocol replies record native identity; Claude retains its supplied ID and OpenCode confirms its native ID through the private server API; `lf __context-block --moment pi-event` retains Pi native identity, activity and cumulative usage; `--moment tool-group` admits the existing watchdog into Pi-owned detached tool groups; Session operations own state | `lf session`, interactive `lf` | `process:cmux` (and tmux, mapped under User) project the attached Session name without owning it |
| **Machine / Promotion** — stable machine identity and artifact selection | `MachineId` is identity; SSH route is mutable. Each person's Machine-local Wave and repository Machine defaults (`lf machine use`), copied to their other Machines, place their unowned Work and, separately, their Wave or repository Session; it is never process ownership or repository content. Promotion owns immutable artifact selection, isolated schema proof, app replacement, and rollback only. Install selects the latest published release independently of caller Git state; the laptop schedule invokes that same command. Checkout updates belong to sync. | [`Machine`](../rust/loopflow/src/durable.rs), [`scope_defaults`](../rust/loopflow/src/store/sqlite/scope_defaults.rs), [`SwitchReceipt`](../rust/loopflow/src/installation.rs), [`published installation`](../rust/loopflow/src/lf/commands/install/published.rs) | `machines`; Machine-local SQLite `scope_defaults`; installation selection and switch receipts; laptop refresh LaunchAgent | The promotion command owns its OS-locked switch transaction | `lf machine`, `lf machine use`, `lf self`, `lf --machine`, `lf install`, `lf schedule` | `process:ssh`, `process:launchctl`, `process:systemctl`, `process:/usr/bin/open`, `process:/usr/bin/osascript`, `process:brew`, `process:/bin/sh` |
| **Rental / Pool** — a machine rented for bounded time and money, or borrowed from an owned pool | The provider's pod is the fact; the rental row records admission (quote, cap, deadline) and state, and a guardian on the pod ends it at the deadline or cap without the account key. An on-demand is a template with a concurrency and hourly-price limit; a pool is owned machines lent one at a time. Giving back copies the machine's records first; `--force` skips that copy. Machines rented by hand stay rented. | [`Rental`](../rust/loopflow/src/rental/mod.rs), [`OnDemand`](../rust/loopflow/src/rental/mod.rs), [`Pool`](../rust/loopflow/src/rental/mod.rs), [`RunPod`](../rust/loopflow/src/rental/runpod.rs) | `ondemands`, `pools`, `rentals`, `machine_records` | The renting command owns admission, boot and give-back; the pod guardian owns the deadline | `lf ondemand`, `lf pool`, `lf machine end`, `lf task run --rent`, `lf task run --pool` | RunPod REST over the Machine row's SSH edge |
| **Session history projections** — captured events and exact provider evidence | LfSession owns provider outcomes and Process owns command outcomes; original payload and exact process receipts confer no Graph authority. | `SessionCaptureSpec`, `SessionCaptureManifest`, `SessionHistory`, `ProviderHistory`, `SessionUsage` | Projects LfSession-owned input/history; Machine-local `runs/<prefix>/<run-id>/` immutable payload and process receipts | shared conversation admission and history | `lf history show`, `lf history replay`, `lf usage`, `lf history`; Work/status history | `process:lf`, provider harnesses |
| **LfProcess** — one actual lf process | The journal transaction records command completion and fixes each child's causal parent at admission. Agent provenance grants no control authority. A detached transport Process owns execution and terminal state; viewers hold separate revocable input tokens. | [`LfProcessId`](../rust/loopflow/src/id.rs), [`AgentCaller`](../rust/loopflow/src/process.rs), [`transport`](../rust/loopflow/src/transport.rs) | `processes` (`kind='lf'`); Session rows reference their transport Process | Foreground commands and detached execution owners (`lf session serve`, launched by `transport::start`); installation/bootstrap coverage remains a cutover obligation | `lf monitor`, `lf history list`, `lf session attach`; ordinary parsed CLI commands | — |
| **AgentProcess** — the provider's OS process | Its original parent and served Session survive attachment transfer. PID/birth and terminal evidence belong to the record; a fresh token fences each attachment. | `AgentProcess`, `SessionAttachment`, common `LfProcess` projection | LfProcess's table, rows of `kind='agent'`; Session retains its current record reference | Provider launch and observed settlement; typed admission remains open | `lf top`, Task execution inventory; LfProcess's process readers | Provider OS process; `process:pi` owns its RPC or native conversation, tools and credentials |
| **Disposable data clone** — offline development data and explicit provider-history archives | SQLite backup and clone-only draft preparation own the disposable schema. A fresh Machine and removed credentials/claims separate retained history from authority; the inherited macOS sandbox confines descendants. Ordinary opens remain validation-only. | [`CloneCommand`](../rust/loopflow/src/data_clone.rs), [usage and proof limits](data-clones.md) | Private clone directory and manifest; copied Machine-local SQLite, captures and archival provider JSONL | Explicit foreground create/run/dispose; no resident service | `lf clone` | `process:/usr/bin/sandbox-exec` |
| **Fake world** — a built world for demos and tests | Recipes in code write through the same ops and store writers as `lf`, so the current schema owns every row; Graphs compile from their authored definitions and Workflows move as `lf task run` and `lf task workflow move` move them. Times are relative to the build; Workflow moves carry build time. Nothing is shared with a real Machine. | [`World`](../rust/loopflow/src/world/mod.rs), [`RECIPES`](../rust/loopflow/src/world/recipes.rs) | New world directory: its own `LF_HOME`, provider homes, git repository and bare origin, a `bin/gh` that answers PR reads from `github/pulls/` and refuses the rest, and `env.json` (with `PATH` putting that `gh` first) | `lf-world` | `lf-world build`, `lf-world list` | — |
| **Local process observation** — recorded processes joined to OS facts | Lf receipts and AgentProcess PID/birth establish observations, not new ownership. Detached records and unknown identities remain visible; failed OS sampling never proves absence. Duplicate or unknown identities grant no signal authority. | [`ActivitySnapshot`](../rust/loopflow/src/lf/commands/top.rs), [`ProcessPruneReport`](../rust/loopflow/src/lf/commands/top.rs) | Process rows and Machine-local lf receipts; no provider registry | Foreground observation and scheduled record settlement | `lf ps`, `lf top`, `lf mon prune`, `lf doctor` | `process:/bin/ps`, `process:ps`, `process:sysctl`, `process:lsof`, `process:kill`, `process:which` |
| **Home disk footprint** — Loopflow's bytes on one Machine and their rotation | Files on disk are the only truth; there is no size budget. Rotation removes backups, draft store copies, captures and logs older than 30 days; the database, account homes, newest backup per database, the last day's material and anything a process holds open never rotate. An unreadable process view stops removal. | [`HomeUsage`](../rust/loopflow/src/disk.rs), [`RotationReport`](../rust/loopflow/src/disk.rs), [`CleanReport`](../rust/loopflow/src/lf/commands/disk.rs) | Filesystem only; removals append to `~/.lf/logs/disk.log` | Foreground disk command, and an hourly background thread in the capturing lf process | `lf disk` | Open-file inspection through Local process observation |
| **Provider account / route** — credential authority and ordered provider selection on one Machine | Provider token/account rows and Access Profiles own routing; credentials stay in provider homes, encrypted storage or Doppler. Machine connection installs a separate resident login. | [`Provider`](../rust/loopflow/src/provider_auth/mod.rs), [`AccessProfile`](../rust/loopflow/src/profile.rs), [`ProviderRoute`](../rust/loopflow/src/profile.rs), [`ProviderAccount`](../rust/loopflow/src/store/mod.rs) | `access_profiles`, `auth_browser_bindings`, `provider_accounts`, `provider_account_limits`, `provider_account_switches`, `provider_routes`, `provider_session_accounts`, `provider_tokens` | The foreground auth command owns provider login process groups and passive browser handoff; durable processes use credentials installed on their Machine | `lf account` | `provider:claude`, `provider:codex`, `provider:doppler`, `provider:opencodezen`, `process:claude`, `process:codex`, `process:doppler`, `process:opencode`, `process:security`, `process:secret-tool` |
| **Conversation context** — refreshed source block and authoring targets | Fixed additions preserve native instructions; skill/request/clipboard form the first turn. Saved source identities reread checkout files at startup and compaction, within 10,000 rendered UTF-8 bytes. Configured budgets measure authored memory/scratch, never gate launch. | [`ContextDelivery`](../rust/loopflow/src/context_block.rs), [`ContextBudgets`](../rust/loopflow/src/prompt/context_budget.rs), [`ContextBudgetReport`](../rust/loopflow/src/prompt/context_budget.rs) | Checkout sources and complete private references under `.lf/prompts/`; no measurement store | `lf __context-block` serves provider startup/compaction hooks; foreground size reporting. Missing optional capture payload does not erase native resume identity; malformed retained payload still fails | `lf context` | — |
| **Code-size measurement** — repository blobs measured in model tokens | Git blob identity owns content; token counts are deterministic memoized measurements, not Run usage. | [`CodeNode`](../rust/loopflow/src/lf/commands/tokens.rs), [`CodeSnapshot`](../rust/loopflow/src/lf/commands/tokens.rs) | `blob_tokens` | Foreground command only | `lf tokens` | — |
| **Store change revision** — which displayed domain a commit changed | Derived inside the writer's transaction by schema triggers, never authored and never bumped at a call site. Transcript lines move nothing; usage moves only its own revision, which Wave detail follows at most every 10 s. The Work reader pins revisions and database-derived joins in one read transaction; nested readers use savepoints. Events only say when to compare revisions. Post-command checkpoints confirm delivery after affected subscriptions catch up, without forcing unchanged queries; explicit Retry still forces reads. OS/Git/file observations are not transactional facts. | [`StoreRevisions`](../rust/loopflow/src/store/sqlite/revisions.rs), [`WorkFrame`](../rust/loopflow/src/lf/commands/work_watch.rs) | `store_revisions` | Store transaction | `lf monitor work` | — |
| **Schema frontier** — ordered definition of durable control storage | Released migration bytes are immutable authority; drafts join only through deterministic release materialization. | [`Migration`](../rust/loopflow/src/store/migration_catalog.rs), [`MigrationId`](../rust/loopflow/src/store/migration_catalog.rs) | `schema_migrations`; canonical and draft migration files | Store open validates/applies; release cut publishes | `lf release` | `process:sh` (release hooks) |
| **Task ownership** — (person, Machine) or unowned | An append-only log; the later entry wins in provider order. Exactly one writer appends, always one of the caller's own Machines: their Machine holding the claim, else their Wave Machine; nothing routes to another person's Machine. Entries travel as marked Task comments. An inbound entry never ends this Machine's own open claim. | [`TaskOwnership`](../rust/loopflow/src/durable.rs), [`TaskOwnershipEntry`](../rust/loopflow/src/durable.rs) | `task_ownership` (append-only by trigger) | The writer's IMMEDIATE transaction, reached over `lf --machine` | `lf task claim`, `lf machine use <machine> --task <task>`, `lf task unclaim`, `lf task run`, `lf task checkout` | Linear assignee; SSH |
| **Portfolio** — repository selection on one Machine | Rust owns folder discovery and manual membership; Desktop reads and changes the same selection. Removing membership preserves files and retained Work. | [`Portfolio`](../rust/loopflow/src/portfolio.rs), `PortfolioMode`, `PortfolioRepo` | Machine-local config.yaml; locked atomic writes | Foreground CLI; Portfolio controls call its API | `lf portfolio` | — |
| **Desktop workspace** — retained presentation of repository, Wave or Task | Work identity selects one repository window and retained workspaces through the sidebar. Pane contents own Machine locations; exact window/content/surface lifetimes fence mutations, which all run through the Swift `DesktopAction` registry. Reads acquire no client and receipts promise no readiness. | `WorkWorkspaceKey`, `DesktopInspection`, `DesktopPaneTarget`, `DesktopActionRequest`, `DesktopActionReceipt` | `dashboards`, `dashboard_arrangements`, `task_orders`, `work_pane_layouts` for saved Dashboards, Task order and portable pane layouts; window-owned registry, multiplexer and native surfaces | Loopflow.app on macOS | `lf desktop` | `$LF_HOME/run/desktop/<bundle-id>.sock` |
<!-- architecture-map:end -->

The public API column covers top-level command families, not every subcommand or
Rust function. [`lf` reference](lf.md) owns argument-level detail. DTOs emitted
by `--json` are required-field projections; Rust/Swift fixture tests own their
wire parity.

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
| Workflows | `task_workflows`, `task_workflow_moves` | The graph a Task took up with its position, and every move, append-only |
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
| Local observation/cache | `blob_tokens` | Deterministic Git-blob token counts |
| Change observation | `store_revisions` | One counter per displayed domain (`planning`, `sessions`, `graphs`, `processes`, `usage`), bumped by schema triggers inside each writer's transaction. Derived, never authored: usage moves only `usage`, and every other Session history row moves `sessions`. GraphProcess rows move `graphs`, a Workflow's position and moves `planning`, and a Session's stream reading `sessions` when availability or reported detail could change; quiet arriving is the reader's clock, not a write. `lf monitor work --watch` reads it to decide which parts to project again |
| Schema | `schema_migrations` | Applied migration identity and checksum frontier |

Released migration files remain immutable. Three direct draft groups establish
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
inbound planning grants no Workflow movement or Process control. GitHub owns PR
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
| `lf wave`, `repo`, `task` | Durable planning and Work coordination | Work resolved in the current planning store |
| `lf session` | Sessions and explicit resolution | Current Machine LfSession state |
| `lf wt`, `commit`, `sync`, `scratch`, `pr`, `ci` | Worktree and delivery operations | Exact repository/Task/GitHub object |
| `lf monitor`, `usage`, `ps`, `top`, `mon prune`, `doctor` | Execution and process observation | Current Machine only |
| `lf machine` | Machine identity | Current Machine unless routed explicitly |
| `lf task claim`, `handoff`, `unclaim` | Task ownership | The writer Machine, reached over `lf --machine` |
| `lf --machine <machine-id> <args...>` | Run the target Machine's `lf` | Explicit remote Machine; no implicit fan-out |
| Loopflow.app | Swift projections and user interaction | Queries the same DTOs and remote routes; owns no lifecycle |

Most commands are local by default. `lf --machine` is transport, not a second API:
the inner `lf` and separator are implicit, the target re-resolves its own Machine
state, and durable processes scrub foreground-forwarded secrets before
detaching.

<a id="harness-launch-and-run-records"></a>
## Harness launch and Session captures

The execution cutover uses one LfSession admission and capture path for Task,
Wave, helper and direct callers.

SQLite owns Process history. Repository trace events live in
`.lf/journal/traces/<trace-id>/events.jsonl` and name their `trace_id` and `process`
node explicitly. Session captures retain the published `~/.lf/runs` directory
as one opaque physical encoding, selected through SQLite artifact keys. Current
and historical captures use the same root: there is no relocation, parallel
layout, alias or privileged conversion. Missing payload does not erase a resumable
Session's SQLite identity. `LF_CAPTURE_KEY` selects subordinate history;
Session/Process caller provenance supplies ancestry and mutation authority.


1. Admit the actual lf Process; resolve typed work without granting Graph authority.
2. Reserve the LfSession and its initial history/capture reference before
   provider launch. Claim its attachment and record exact publication state.
3. Publish immutable input atomically. An unpublished reservation is recoverable;
   uncertain publication/spawn evidence never permits a blind duplicate launch.
4. Start the AgentProcess, or connect to the one a live attachment holds. Record its
   identity and endpoint, distinct from the client's process and the attached LfProcess.
5. Append correlated provider outcomes and usage; retain missingness. The
   Graph process reads a deciding or routing step's answer from the turn that step's
   Process captured.
6. Settle the actual command's Process when the process completes, independently of
   whether its conversation remains open.

### Retained encoding inventory (LOO-370)

- `session_record::record_dir`, active-reader watches, ablation staging and
  preservation fixtures retain `runs/<shard>/<artifact-key>`. This is the single
  physical capture root; keys (including historical `run_` strings) are immutable.
- `ops/git_operation.rs` keeps serialized `run_id` / `lf_process_id` receipt fields
  while Rust names their actual Trace/Process owners. Released SQL, migration
  fixtures and `session_events` historical source labels retain their original
  bytes. Historical cohort readers in `scripts/context_ablation.py` still decode
  `launch`, `run_id` and `parent_run_id` from their frozen inputs.
- `LF_RUN_ID` / `LF_RUN_DIR` occur only in launch scrubbing, rejection fixtures
  and the checksum-pinned released-CLI preservation fixture. Current execution
  reads `LF_CAPTURE_KEY` and typed Session/Process provenance. Capture context does
  not confer Graph or Task settlement authority.
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

## Tracked Work and Task Graphs

```text
lf task run ISSUE [GRAPH] -> place the worktree, take up the Wave's workflow or repository default
                         -> lf --task ISSUE graph GRAPH as a child: one lf process,
                            started again while a Graph process fails (three at most)
                         -> step processes, one child process each
                         -> current, completed or stopped
```

Every Graph naming a Task is equally its work; none is selected over
another. `task run` returns to its launcher once its Graph process registers,
leaving output in `logs/graph-<id>.log`; `-b` blocks and prints. The Graph process holds the cursor in memory and
advances it from each step's exit; the position a step ran at is on that step
Process's argv.

A Graph process that dies leaves dead Processes as history. Nothing restarts or resumes
it. Liveness comes from OS process evidence, and missing evidence stays uncertain.
Only live or unknown Processes retain checkouts and exact controls; a provider
turn without a completion is history. Execution does not veto authorized Task
completion or cancellation. Git/PR delivery still needs its own evidence. Fresh
work starts another Graph after inspecting existing execution.

Independent helpers may carry the same Task and independent Git/PR
operations. Binding to done Work assigns a conversation without reopening it.
Legacy review boundaries remain readable history when their Work is terminal.

Chapter rotation preserves Task, LfSession, PR and worktree
identity when moving started unfinished Tasks. Local selection, membership and
pending effects commit atomically across the selected Waves. Provider delivery
converges independently; there is no cross-system atomic switch.

## Durable communication

```bash
lf comment INF-123 "keep the public name" --steer
lf wave comment infrastructure "preserve native history" --steer
lf repo comment "retain source attribution" --steer
lf session steer SESSION "inspect the last failure"
lf session steers SESSION --json --limit 32
lf session steers SESSION --after 42 --json
lf session steers SESSION --source STEER_ID
lf session steers SESSION --retry STEER_ID
```

Steers retain immutable source revisions and authorship. Task comments and
definition acquisition save steers in their existing planning transaction; their
Linear delivery receipts remain separate. Repository and Wave steers are
Machine-local and report unavailable shared planning exchange for those scopes.

Each Session selects its steers from its repository, Task association and
Wave ancestors. Peers receive independent receipts. Saving a steer starts no turn;
an idle Session receives pending steers on its next supported provider boundary.
Startup, user continuation, tools and compaction share the receipt path. Task
brief references contain no consumed steers.

The provider capability belongs to the selected AgentProcess and native
conversation, survives client attachment handoff and fails after replacement.
Child callbacks cannot consume parent steers. Shared Session dispatch exclusion
lets startup hooks finish while native stdin is blocked; takeover remains exclusive.

A boundary reserves at most eight steers and renders at most 10,000 bytes
including reference context. Oversized text has a marked UTF-8 excerpt and a
complete immutable source. Only the selected page is materialized.
Receipts distinguish pending, prepared, uncertain and emitted/unconfirmed.
Successful stdout proves local emission only. No current adapter manufactures
confirmed acceptance. Explicit retry creates another attempt without waking work.
An exclusive, nonblocking Session fence proves a prepared emitter has exited
before its reservation is cancelled; elapsed time grants no retry authority.

OpenCode gives each context hook an adapter invocation UUID. Internal stream retries
reuse that model request; later hooks add fresh system context
and do not reselect emitted steers. Providers without an observed authenticated
callback report that limitation; already-open providers without declarations need
a provider start.

### Questions and sessions

These examples use current command spellings; the lifecycle contract is above.

```bash
lf -b implement
lf session list --interactive false --task INF-123 --json
lf session connect SESSION
lf session rename SESSION "Migration review"
lf session bind SESSION --task INF-123
```

Every conversation has one LfSession regardless of launch surface. Connect
uses the existing AgentProcess where possible; client replacement retains its active
turn. Name, feedback, native identity and history survive reconnect and recovery.
A suggested title cannot overwrite a human-assigned title. CLI and Desktop use
the same action and availability reason.

Ready saves feedback and keeps the conversation open. Complete persists its
closed state and exact feedback before teardown. A Graph review returns
feedback for the following decision; it never selects that decision's edge.
Pane close, provider exit and readiness do not complete the review.

The desktop terminal pool keys on stable LfSession identity. Rename, bind,
reconnect and replacement retain the pane and draft when reusing that surface.
Recovery after AgentProcess exit does not claim preservation of text never submitted
to Loopflow without separate UI evidence.

## Graph execution

```bash
lf task run INF-123                  # Project's Graph
lf task run INF-124 incident  # explicit override
lf graph example                    # with or without Task attribution
```

The Graph process compiles the graph, every router and Xor alternative, and holds it
with the cursor and return counts in memory. Definition changes affect new
Graphs. A past Graph keeps the graph its Graph process recorded at launch. Backward edges stay finite graph structure; future loop passes are
not preallocated. Node IDs are local typed identities, not path strings.

Taking an Iterate edge updates the Graph process's cursor and return counters. The
node key and per-edge iterations on each GraphProcess step row identify the pass,
including overlapping backward edges. Neither loop passes nor authored subgraphs
create Graphs or lifecycles.

Task and taskless execution use one Graph runner and need no synthetic
planning records. The draft migration archives each earlier Graph row's name,
state and last cursor as a `legacy_flow` observation on the Sessions it opened,
then drops the Graph tables and the Session's Graph column. Configured acceptance
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

A Task binds planning to one managed checkout and one current PR, when published. Placement owns
its branch and base independently of publication; a research Task needs no PR.
A checkout on that branch identifies the Task regardless of its upstream; the
stored path supplies placement for explicit Task selection. Publishing continued work after a PR settles retains the previous PR as read-only
history and selects the new PR. Recommend a follow-up Task for a distinct outcome;
continuing in the same Task remains supported.

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
                         optional ordinary Task creation
                                       |
                              complete Task
```

1. `lf checkout` resolves one Linear Issue inside one Project and creates
   or reuses Task Work and its checkout placement without creating a PR. It starts no
   agent work.
2. Independent `lf --task ...` conversations may work in that substrate directly.
   `lf task run` places the Task, defaults its Graph, then runs it through the
   same `lf graph` path as any other launch.
3. `lf commit` snapshots the worktree. `lf pr publish` creates or refreshes the
   current PR without opening a browser.
4. `lf submit` leaves the exact-head merge click to a person. `lf arm`
   and `lf land` request exact-head auto-merge, record the landing, and
   return; `lf pr reconcile` checks recorded landings once. All operate on Task delivery state when it exists and require
   no Graph-driving claim or execution receipt.
5. PR landing is fenced by landing generation. A check repairs an unchanged
   incident once; a moved head requires fresh evidence. A blocked landing still
   observes merge, and explicit arm or land resumes it.
6. Verified merge alone leaves an open Task available for optional new Task creation.
   No follow-up disposition is required. `ship` runs gate and `land --wait-and-fix`;
   its Workflow endpoint requests completion. A stopped Graph leaves the operator
   to inspect live work and retry completion alone when the outcome is satisfied.
7. `task complete` changes status without moving the Workflow. Arrival at end
   atomically requests completion; a failure retains arrival and retries only
   completion. Accepted Linear completion/reopening leaves execution intact and
   supersedes old completion intent. Ordinary reads execute no trigger. A PR-less Task
   may finish with retained artifacts; a Task with a PR needs authoritative
   merge. Dependent PRs belong to stacked Tasks.

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
AgentProcess record. Revalidate that evidence before every signal.

A headless AgentProcess lives only while an LfProcess attached to it does. It
runs in its own process group with a watchdog that holds the read end of a
lifeline; each attached LfProcess holds a write end until it ends. Return, a
signal, a panic and SIGKILL all close it, and the watchdog then terminates the
group. Only Codex names its lifeline, so only a Codex takeover can hold the same
lifeline first and leave the AgentProcess alive; takeover coverage for other
providers and foreground terminals remains open. A saved endpoint alone is not
liveness; a missing endpoint alone is not death. An LfProcess that replaces a
dead one ends the AgentProcess it left by recorded PID and start time, then
resumes the saved native thread on a new AgentProcess; the turn that was in
flight is not recovered. Nothing authorizes terminating a shared AgentProcess to
recover one thread.

Pi's native bash executor creates detached tool groups. Its launch extension
wraps the built-in executor with a per-command FIFO writer held by Pi. Before
user commands run, `lf __context-block --moment tool-group` verifies that the current detached group
is a direct child of the live provider, then starts the same group watchdog.
Normal completion, abort and provider death close the writer. Custom extension
tools retain their own execution ownership; there is no tool PID registry or
postmortem discovery of signal targets.

One exception to local-handle control exists. The scheduled repository check and
`lf mon prune` terminate an AgentProcess that outlived its attachment: the recorded PID and
start time still name a live provider server leading its own process group, and
every Session naming it is headless with an attached LfProcess that is provably dead.
Unknown evidence, a live attachment, an interactive Session or a reused PID leaves it
running. The Session records the exit and why, and its AgentProcess identity is cleared.
Task or Wave membership still grants no authority to stop anything.

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
                                      LfSession native history
```

Wave operations are finite conversations. Tasks drive their selected invocation; finite
`lf pr reconcile` checks observe delivery. Crossing Machines is an explicit `lf --machine` hop
whose target proves its Machine identity.

### Multi-Machine ownership and execution

`MachineId` is stable identity; its SSH route is replaceable. Which Machine runs a
person's Wave work is that person's choice, never repository content:

```bash
lf machine use mini infrastructure            # this Wave's new Tasks and its Session
lf machine use laptop infrastructure --session  # only the Wave Session
lf machine use mini --repo --tasks             # only the repository's new Tasks
lf machine list                               # each Machine with the defaults naming it
```

The setting lives in Machine-local SQLite `scope_defaults`, keyed by Wave or repository, and is copied to the
person's other added Machines over `lf --machine`; an unreachable Machine gets a
warning and a later rerun. Resolution for an unowned Task walks its Wave, its parent
Waves, its repository, then this Machine, so single-Machine use needs no setup. Cron install reads
the same resolver. A scope's Session Machine is a separate setting with the same
resolution (`--session`; `--tasks` sets only the Tasks'). Setting it completes the
old Machine's Wave primary Session and admits a fresh one on the new Machine; native
provider history stays behind. `lf session connect --wave <w>`, bare connects that
resolve to a Wave or the repository, and Desktop's Wave Session route to the Session
Machine. A Task's Machine is captured in its claim: changing the Wave Machine moves
no existing Task, and `lf machine use <machine> --task <task>` moves one, completing its primary Session
on the old Machine.

```bash
lf machine add jack@mini.local --label mini
lf task claim LOO-523
lf machine use mini --task LOO-523
lf task unclaim LOO-523
```

A Task is owned by (person, Machine) or unowned. Its `task_ownership` log holds
`claim`, `handoff` and `unclaim` entries that are never updated or deleted; the
later entry wins in provider order (Linear comment time, or the Git planning
journal order, where each local append sorts after every entry its Machine has seen).
A handoff moves the claim to its target Machine and keeps the person; it needs
only the target's identity, not a connection.

Forwarding stays within one person's Machines. A Task the caller owns on another
of their Machines reruns there; an unowned Task gets the caller's claim on their
Wave Machine; a Task someone else owns gets the caller's claim on their own Wave
Machine, and the later claim wins. Nothing routes to another person's Machine, and
nobody hands off or unclaims another person's claim; cross-person handoff is
deferred. The Machine in a claim comment is informational only.
`lf task claim|handoff|unclaim`, `lf task run` and `lf task checkout` route to
that writer over `lf --machine` and re-check inside one IMMEDIATE transaction.
`task_execution_route` returns the claim's Machine (source `owner`) or the Wave
Machine (`owning_machine:wave:<name>`, `owning_machine:this_machine`). `lf task location --peers` observes other
Machines' checkouts and routes nothing.

Each local entry is delivered as a marked Task comment ("Jack Heart claimed this
on mini" plus a `loopflow-ownership` HTML comment carrying the entry) through
ordinary comment delivery to Linear or the Git planning journal; inbound comments
are parsed into the log. With Linear, a claim sets the assignee to that Machine's
Linear viewer and moves backlog, triage or `unstarted` to `started`; unclaim clears
the assignee; handoff leaves it. Assignee changes write no entries; the owning
Machine re-asserts its assignee while its claim is unsuperseded.

An inbound entry never ends this Machine's own open claim; only its own unclaim or
handoff does. When another Machine claims later, every other Machine shows the
later claim while the earlier claimant still owns it locally, reports
`superseded_by` and steers its live Sessions. Nothing is cancelled. If the log
stops showing this Machine as owner for any other reason, such as a stale
Machine's unclaim, the owner claims again.

The target uses its own store, repository, provider homes, OS locks and payload
directory. Reads remain Machine-local. Foreground SSH may explicitly forward
selected account authority; detached processes use installed credentials.

Wave selection always resolves `(canonical repository, slug)` to one UUID.
Bare-slug diagnostics fail when more than one repository owns the slug; no
read or mutation chooses one by order. A scoped lookup repairs an equivalent
legacy path spelling to the canonical repository in one transaction.
`lf wave rename <uuid>` is the only semantic locator mutation: it fences
the Wave chord, moves authored files, commits the new locator
transactionally, and leaves PM and Work rows joined to the
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
| [Linear acquisition](../rust/loopflow/src/linear/acceptance.rs) / `pm_projects`, `pm_items`, `pm_wave_projects`, `pm_wave_sync`, `pm_issue_changes` | Linear planning | Shared entity ingestion orders provider revisions and retains last-good facts when refresh fails. Canonical Task and Wave views read saved Work with separate acquisition evidence; ownership checks read retained claims directly. Inventory and detail acceptance consume concrete Linear fields in one transaction; there is no public snapshot envelope. Observation age and invalidation remain explicit. |
| [`TaskLinearObservation`](../rust/loopflow/src/work/task/mod.rs) / `task_linear_observations` | Linear Issue title, description and comment revisions | Reconciliation saves comments and appends deduplicated direction. Team acquisition also accepts issue state and Project membership through the typed Linear owner; the saved comment projection grants no Workflow or Process authority. |
| [`GithubObservation`](../rust/loopflow/src/work/task/mod.rs) / `task_prs`, `ci_incidents` | GitHub PR/check state | Webhook or foreground reads update Task delivery evidence; GitHub remains merge truth. |
| `tests/fixtures/dto/` | Rust `lf --json` DTOs | Rust and Swift fixture tests reject required-field or enum drift. |
| `tests/fixtures/migrations/` | Ordinal-free migration drafts and the Python canonicalizer | Rust build/runtime and Python release tests reject ordering, body-byte, checksum, and graph-error drift. |
<!-- architecture-projections:end -->

`lf wave status` and `lf wave show` derive Task conditions from Work, Process and
PR facts. LfSession and Process own ancestry; immutable history keeps
earlier attribution. The transitional joins are listed in the cutover status. No projection acquires launch, Work-mutation,
credential or signal authority. UI grouping is cached presentation, not another
attribution store.

## Extension rules

| Area | Safe extension | Architectural constraint |
| --- | --- | --- |
| Execution queries | Add an index for a measured Process or LfSession query | Filter before payload IO; identity/ancestry come from the owning row, never fallback files |
| Multi-Machine views | Fan out read-only commands through `lf --machine` | Do not centralize Machine-local execution ownership or silently mix local and remote scope |
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
| `shim:retired-app-replacement` | Promotion removes the previously shipped app bundle after the current app commits. | [`AppPromotion`](../rust/loopflow/src/lf/commands/install.rs); remove after the retired bundle name is outside supported installs. |
<!-- architecture-shims:end -->

## Appendix: historical-only vocabulary

The scanner matches exact phrases, not overloaded words. Provider resume
sessions are current. The authored chat
reference `project:<slug>` is also current; it is not the old Linear-label PM
model.

<!-- architecture-vocabulary:start -->
| Retired term | Allowed scopes | Current language |
| --- | --- | --- |
| `Project Session`, `project_sessions`, `task_sessions` | `rust/loopflow/src/store/migrations/`, `rust/loopflow/src/store/migrations.rs`, `rust/loopflow/src/store/tests/fixtures/`, `release/` | Stable Project/Task **Work**; LfSession owns the conversation and native history; a Graph is a Graph process and its step processes. |
| `session context`, `LF_SESSION` | — | Typed Work ancestry and Process/LfSession provenance; transitional launch environment names are listed in cutover status. |
| `lf radio`, `agent bus` | `release/` | Typed Work observations, Steer, synchronous questions, and review GraphSteps. |
| `pm.linear_project`, `projects/<slug>.md` | `release/`, `rust/loopflow/src/ops/project.rs` | `linear.initiative`; Linear Initiative → Project → Issue. The Project operation reads the historical YAML field once into SQLite and retains original bytes; later file edits are inert. |
| `machine-local host`, `machine-global command`, `machine-global mutation`, `machine-global reservation` | — | Machine-local keeper, command, mutation, or reservation. |
<!-- architecture-vocabulary:end -->

The following map gives the destination of retired or transitional representations.
Cutover status identifies which runtime consumers remain; the map does not claim
their deletion is finished.

| Historical or transitional representation | Target owner |
| --- | --- |
| Wave-scoped Chapter / `wave_chapters` | Linear Project status; Chapter is the shared In Progress name, with no stored object |
| Recommended Graph / `graphs.recommended` | Wave or repository Workflow default |
| `GraphPosition`, `PinnedTaskGraph`, `task_graph_positions` | The Graph process's in-memory cursor; GraphProcess records the graph and each step's node |
| `GraphRun`, `graphs/<id>/position.json` | The Graph process and its step processes |
| Subject selector list on a Run | Typed LfSession/Process ancestry and immutable event attribution |
| Four Session projections, Ask files, composite boundary Session IDs | `agent_sessions`, keyed by stable Session ID, with attached LfProcess and native history |
| Session name/resolution and provider attachment sidecars | LfSession attributes, native identity and exact process evidence |
| Step occurrence / path-string node key | Graph process ID, local node key and iteration tuple on the GraphProcess step row |

Canonical migrations, migration fixtures, and release notes retain historical
names because changing shipped evidence would rewrite history. Operational docs
and current runtime source do not. Dated operational archives belong outside
the tracked source tree.

## Authority and failure invariants

- SQLite owns planning with optional Linear synchronization; observed Linear
  conflicts win while losing intentions remain history. A Chapter is the shared
  name of one In Progress Project per Wave; uncertain delivery stays visible.
- Wave instruments and observations survive Chapter changes. Missing target
  planning is unknown, not proof that an instrument is untargeted; it cannot
  erase an observed reading.
- An agent launch needs no planning parent, but requires writable admission and
  immutable captured input before provider side effects.
- LfProcess is an actual lf process. LfSession history owns native outcomes and
  usage; a Graph is a Graph process and its step processes, whose exits are its results.
- Command outcome, native completion, current liveness and Work disposition are
  separate facts. Missing terminal evidence stays unknown.
- Causal parentage and Task attribution confer neither process control nor Graph
  settlement. Exact process identity and native scope govern signaling.
- An authoritative conversation attachment is singular; its claim token differs
  from provider identity. Passive readers acquire neither claim.
- A Graph consumes its selected successful native completion under version/claim
  fencing. Failed turns cannot donate verdicts or routes to successful retries.
- Complete closes a conversation or returns review feedback. Readiness and provider
  exit never choose an edge; a following decision owns its own navigation.
- Bind is write-once, same-target idempotent and valid for done Tasks. CLI states
  the permanent target and writes; Desktop confirms. Historical attribution and
  the ownership log survive assignment and attachment changes.
- OS locks cover their named critical sections. Sync admits only its exact
  recovery child; causal identity does not bypass Git or PR mutation authority.
- Store copies preserve evidence without acquiring process authority. Promotion
  and historical-writer continuity remain separate proof obligations.
- Reads are Machine-local unless explicitly routed by `lf --machine`. Indexed summaries
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

A Task Session is an LfSession associated with a Task checkout. A Task can have
any number; repo and Wave Sessions retain their explicit scopes. This term does
not restore the retired Task Session controller or a second conversation store.
`session list --orphan` selects Sessions without Task association; it cannot opt
a Session out of checkout membership.

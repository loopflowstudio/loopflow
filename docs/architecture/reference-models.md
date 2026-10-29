# Models and vocabulary

Part of the [Architecture Reference](../architecture-reference.md).

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
| Task, Project, repository, Machine, PR, landing, tool response | Lowercase prefix and UUID (`task_`, `proj_`, `repo_`, `home_`, `pr_`, `landing_`, `response_`) |
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

These are the accepted product owners; see [cutover status](reference-cutover.md#cutover-status)
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

`GraphCatalogEntry` carries the compiled graph and `GraphComposition`
disclosure tree. Invalid sources remain visible with their error.

`lf wave graph show/set` reads or changes the Wave's default graph,
independently of Project history.

Task status and roadmap separate `graph_name` with its `graph_source`
(`task`, `wave`, `repository` or `builtin`), `latest_graph_process`,
`execution` and `run_control`. Task work and overview carry `graph`, a
`TaskGraph` of name, interactive steps and position. The latest
`GraphProcessDetail` is ordinary execution evidence, including completed graphs,
not a second Task lifecycle. Task work and watch carry `graph_processes`. Session
membership references the exact `graph_lf_process_id`; a Task graph's run names
its Task-run `lf_process_id`. Persisted graphs and historical capture formats
remain unchanged.

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

One actual lf process that changes state gets one LfProcess, including nested
direct and agent-issued commands. A read (status, list, show, diff, file, every
`--watch` reader) records none, so it never wakes a Work reader or joins a Task's
work. `Commands::changes_state` classifies every command once, exhaustively. In-process wrappers reuse it and cannot settle it early; each Graph
step is a child process with its own Process.
The incoming `via_agent` bit describes the caller, not whether the command later
launches an agent. Agent-issued children resolve stable Session/AgentProcess
provenance to the current attached LfProcess once, at admission. Delayed children
of a replaced provider retain their historical origin; existing parents never change.

Process outcomes are succeeded, failed or interrupted when observed. Unobserved
completion, exit code and signal stay unknown. The outer command owns terminal
settlement, including interruption. Bootstrap/installation logging must not open
or migrate an incompatible installed store before its authority preflight.
Parser errors, `--version` and installation entry paths append process evidence
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
Projects retain history. A Task without a graph takes up an explicitly named one, its Wave default, repository `graph:`, then builtin `code`.
There is no Chapter row, packet or local switch. Rotation uses an explicit target
and stable Project identities, converges after partial mutations, preserves active
Task identity/worktree/PR/execution, and refuses unrelated competing plans.
A second Machine adopts the same state through ordinary synchronization.

Adjacent APIs keep their own authority: Task PR operations own Git/GitHub;
provider routing owns credentials; Task ownership owns routing; exact process
receipts and current OS evidence own signaling. None follows from causal ancestry.

# Models and vocabulary

Part of the [Architecture Reference](reference.md).

## The system grows outward from a direct Skill launch

The direct Skill launch is the kernel of Loopflow. It remains useful with
no Wave, Task, or server process. Agent admission requires its Machine's writable store. Higher layers
supply composition, durable context, delivery, ownership, and views around its
discovery, prompt, provider, harness, and evidence components. Their
boundary executors remain domain-specific because their settlement rules differ.

```text
                        +--------------------------+
                        | UI and read projections  |
                        | status / planning / app  |
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
  branches and follow backward edges. `interactive:` marks the steps where a
  person takes part; Task runs stop there for their conversation.
- **Durable planning:** preserve Wave and Task intent across crashes;
  each Task Graph is a fresh Graph run and the steps it starts.
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
 Wave -> Task                   LfProcess / LfSession
       stable Work           indexed history + payloads
          |
          v
 repository + Git worktrees

exact local races use OS locks; remote execution uses lf --machine
```

- **Tracked Work** records purpose, input and convergence; each Graph run
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
Wave --< Task --< Graph run       a Wave's planning binds one Linear Project

LfSession --- AgentProcess         one current provider, recorded on the Session
LfSession --> attached lf          nullable current attachment, fenced by token
LfSession --< history              immutable provider outcomes and usage
Graph run --< step                 graph_process_steps; result is the step's exit
```

| Concept | Owns |
| --- | --- |
| Repository | Wave membership |
| Wave | Enduring objective, memory, cadence, budget and metric instruments; its planning holds status, Tasks, KRs and targets (one Linear Project when connected) |
| Task | Worktree, optional PR, captured graph and position, and every associated Session and Graph run; independent Graphs do not move that position |
| Graph | Reusable authored graph of agent/mechanical/router nodes; a running one is a Graph run and its steps |
| LfProcess | One lf invocation's opaque id (`LfProcessId`, `LF_PROCESS_ID`); no table holds it. The record that owns the work (Session attachment, Graph run, step, Task carrier, CI repair, project activation) carries its pid, OS start and result |
| AgentProcess | The provider OS process Loopflow started: durable `id` (`AgentProcessId`) with its launching lf, pid, OS start, spawn state, exit, endpoint, inbox and socket, recorded on its Session's row; the Session also holds the attached lf, that lf's pid, OS start and terminal, and the attachment token. A Session runs one agent at a time; a replaced one has exited |
| LfSession | Loopflow conversation identity, title, feedback, current AgentSession and provider history |
| AgentSession | Provider-owned conversation, identified by opaque AgentSessionId for resume, turn matching and account attribution; no separate table |
| Machine | Store, payloads, credentials and exact local process authority |
| Task ownership | (person, Machine) or unowned, read from the Task's append-only ownership log; no authority over merely observed processes |
| Steer publication | Immutable authored direction with independent Session delivery |

Alive is one OS lookup against the owner's pid and OS start:
`process::os_process::RecordedProcess::evidence` is the rule, and
`step_evidence`, `carrier_evidence` and `session_agent_evidence` are its owner
cases. The `processes` table is leaving (LOO-965); only a released lf writes it,
and the expand migration's triggers copy those rows onto their owners.

A skill invocation keeps one `LfProcess` identity while retries replace its
`AgentProcess`. `process::LfProcessRuntime` is the shared in-memory resource
handle for that work: its private state holds the current attachment, provider
thread/account and outcome. It has no separate execution ID or persisted
lifecycle. Helper requests inside a mechanical command can scope their own
runtime resources under the same command provenance.

`session_record::SessionRecorder` publishes the captured input and writes attempt
history and the terminal receipt. It cannot launch or stop a provider. Native
exit adopts only its recording position through `CaptureHandoff`; attachment
cleanup remains with the process owner. Runtime cleanup runs before receipt
writing, so a recording failure cannot skip cleanup or stop a replacement.

## Definition and execution projections

`GraphCatalogEntry` carries the compiled graph and `GraphComposition`
disclosure tree. Invalid sources remain visible with their error. `included_by`
names the Graphs that include it with `graph:`, so a client can tell a piece
from a Graph people start with, and `runs` says whether it runs for a Task or
a Wave. `workflow` is a `WorkflowOutline`: the top-level steps, the stops and
the take-part sentence a `TaskGraph` carries, for a Graph no Task stands on
yet.

`lf wave graph show [WAVE | --no-wave]` reads the graph a Wave's, or the
repository's, new Tasks take up and which setting chose it, independently of
planning history; `lf set wave.graph` changes it.

Task status and `plan list` separate `graph_name` with its `graph_source`
(`task`, `wave`, `repository` or `builtin`), `latest_graph_process`,
`execution` and `run_control`. Task work and overview carry `graph`, a
`TaskGraph` of name, interactive steps and position. The latest
`GraphProcessDetail` is ordinary execution evidence, including completed graphs,
not a second Task lifecycle. Task work and subscription carry `graph_processes`. Session
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
| `processes` | Leaving (LOO-965). Written only by a released lf during the upgrade window; triggers copy its rows onto the owning Session, Graph run or step, and agent rows are copied back for a released holder's fence. No reader in this lf except the released-lf skew inventory. |
| `agent_sessions` | Stable ID, purpose and independent interactive flag; title/provenance, request/feedback, typed Task/Wave ancestry, the scope it is primary for (`primary_scope`: repository or Wave), native identity, the current AgentProcess and its attachment; reserve, attach, resume, bind and rename; historical feedback/completion retained |
| `tasks` | Shared planning row: identity, repository, project, title, description, state, rank and tracker revision; no Machine-local column |
| `task_checkouts` | This Machine's checkout of a Task (worktree path, branch, base commit, parent PR); absent on every other Machine |
| `task_primary_sessions` | The conversation this Machine opens for a Task, with or without a checkout; absent on every other Machine |
| `task_ownership` | Append-only `claim`/`unclaim` entries (`own_…`; `lf task unassign` writes `unclaim`, shown as unassign) with person, the Machine they name, the Machine that wrote them and time; a move is a claim naming another Machine, and earlier `handoff` entries read as one |
| `projects` | A Wave's plan: stable local identity, optional provider binding, status, prose, KRs and targets; the default graph belongs to the Wave |
| `waves` | Stable identity and repository address; objective, memory and instruments are read from checkout files |

### Conversation and attachment lifetime

Every agent conversation is an LfSession: skills, inline prompts, helpers,
reviews, interactive and headless work. Default views select unfinished interactive
conversations. `lf session list --headless` adds unfinished headless Sessions;
`lf history` owns finished conversations. `lf session list --available` selects
conversations Available for a person, using the same Rust projection as Desktop. `session_activity` holds both
the attached LfProcess's latest stream reading and an optional terminal-reported snapshot.
Its optional preview carries bounded provider text, native thread/turn identity,
source and observation time. Only the current live AgentProcess and attachment
can project it. Provider selection changes clear it; a new Codex turn clears the
previous excerpt before output arrives. Desktop applies a 30-second working-text
lifetime without changing execution state; current Available results retain their
source age. Missing or disconnected evidence uses a labeled summary.
A recorded AgentProcess alone decides whether its Session is active: completed
means closed. One whose end is recorded is dead by its own row, in every read
and for a finished Session too; the OS is asked only about agents that have not
ended, where exact PID/birth evidence is live, dead or unknown. Unknown
is never Available and never ages into replacement or checkout-cleanup authority.
Every owned launch records an AgentProcess before exec, including native terminals.
Sessions without one remain unknown; client files are not read. A held
Session names its terminal in `held_terminal_id`; `transport_lf_process_id`
names an owner only for a conversation an earlier build started, which
`lf resume` restarts held. The execution child owns capture settlement and the
frozen provider attachment; viewers receive separate revocable input tokens.
Passive viewing takes no claim. A view is offered only for a live held
terminal; a connection failure is an error, not permission to start another
agent. Input completion uses
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
panes keep local records; Session panes send each bounded snapshot to the
server as one `session.status` write with the current terminal receipt and the
reading's AgentProcess, on the connection Desktop already holds. No process
runs per pane. Stream identity and increasing sequence fence replacements in SQLite.
A replaced agent's reports end with it. Report text is literal
and display-sanitized; protocol IDs never select Sessions or grant process,
Graph or completion authority. Detached relay observation remains unimplemented.
`--headless` includes headless Sessions; `--all` means all repositories. Interactive mode grants neither review
completion nor Graph authority.

A primary Session is the one ongoing conversation of a scope. It is an ordinary
interactive conversation whose row names that scope; a repository and each
Wave have at most one uncompleted primary, and a Task one primary. The repository's needs no Wave,
Task or tracker. `lf resume` finds or admits it, prepares its
workspace, reserves the first input and owns the foreground launch; `--prepare`
stops before launch. Task primaries are admitted lazily on the owner Machine, on first resume;
explicit Wave admission also admits the repository conversation. A Session may
have no current capture: metadata admission does not claim a Task, prepare
a prompt or start a provider. The first captured input, or binding a previously
captured conversation, starts work.
A primary is never replaced: `lf session new` appends a fresh input to the same
row, seeded with a log of its current AgentSession, which stays in its history.
`lf resume --fork` admits a separate, non-primary Session from that log.
Scope locks serialize admission. Primary grants no Graph, review or process authority, and
the Wave's goal and memory reach it as ordinary Wave context.

A Task's primary is one of its own conversations, named by its row in
`task_primary_sessions`, so it stays a Task member; reads report its scope as
`task`. Choosing another replaces the row. Released lf reads the same table;
LOO-877 moves the mark onto `primary_scope='task'`. `lf resume --task ISSUE` keeps the selected primary. Without one, it
selects an unfinished interactive member by latest native human input, falling
back to interactive opening and creation time; it creates one when none exists.
`lf session bind` selects the primary explicitly. Opening an exact Session leaves
an already selected Task primary unchanged.

AgentSessionId preserves the provider's exact id, including non-UUID ids. The
selected AgentSession belongs to the LfSession row; the provider endpoint belongs
to the AgentProcess. Captured tracker events retain native resume references.
A replacement preserves LfSession identity. Immutable turn/capture history keeps
earlier AgentSessions and their account attribution: replacement does not
reassign old usage. A new AgentProcess resumes the selected AgentSession.
No reader needs an independent provider-conversation table.

SQL columns and captured JSON retain `provider_thread` / `provider_session_id`;
Rust uses `agent_session` and Swift uses `agentSession` with the same string
encoding. This preserves existing native evidence and DTO fixtures without
a naming-only migration. Provider protocol names remain vendor-owned.

An LfSession can have many historical attachments and at most one current
attachment. Compare-and-set issues a fresh opaque token on every claim and release,
including A → B → A. The token has no independent lifecycle. Every claim starts
a new AgentProcess, so a claim replaces the agent only after it exited or never
spawned; a released agent keeps its identity and launcher until then. Old clients may retain provider history but cannot start/steer turns or
change current attachment, connection, process evidence or stream activity.
History retention grants no native-write or Graph authority. Passive connection
acquires no claim. Dispatch fences include queued native RPCs and approval replies, not only database claim updates.

Admission reserves an AgentProcess row; spawn request, exact OS identity and
observed exit update that record. Historical provider receipts remain immutable
history, not current lifecycle authority. Missing identity or exit stays unknown;
Lf completion never proves the AgentProcess exited. Saved native thread identity
is loaded before native launch. Historical account attribution does not select credentials.

Native dispatch and attachment claim, release, and exit share a Session-scoped OS
lock beside the canonical database path. Attachment validation releases the SQLite
mutex before provider I/O; history and other Sessions keep using the database.
Lock acquisition has an OS-clock deadline independent of the provider reactor.
Lock files retain their inode across process exit; deleting a live lock file
would let two processes own different locks for the same Session. The Session's
immutable `fence_locator` retains that address independently of its ID spelling.
This lf writes no Process row or receipt, and names a trace's journal
directory by its trace id.

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
when that LfProcess launched its AgentProcess. Interactive Task/Wave conversations, primary
Sessions, Asks and Graph reviews remain available. A headless conversation
completes when its AgentProcess exits, whatever its Work, purpose or Graph:
completion means it is no longer doing work, not that it cannot continue.
A new AgentProcess on it, such as a Graph correcting its own step, reopens it
until that process exits. Pruning a dead AgentProcess also completes its headless
Session. A stopped turn alone never completes a conversation, and retirement
never settles a Graph.
Completed Sessions retain their history but contribute no current rows or counts.
Terminal Tasks cannot acquire current review obligations;
independent unresolved Asks remain discoverable.

LfSession history records provider starts, successful/failed/interrupted
outcomes, retries, durations and usage, correlated with driving lf and native
turn/receipt. Repeated receipts are idempotent. Missing measurements differ from
zero; cumulative samples are not added together. Later continuation never rewrites
an earlier completion a Graph already read. Provider completion and Process
completion are distinct: subsequent command work may fail after the agent succeeds.

### lf invocations

An lf invocation that changes Work (Tasks, Waves, Sessions, Graphs, PRs,
planning) gets an `LfProcessId` and a journal trace
(`.lf/journal/traces/<trace_id>/`, plus `logs/lf.log`); no table records it. A
read (status, list, show, diff, file, every `--subscribe` reader), Desktop
navigation and pane or Dashboard arrangement get no trace, never wake a Work
reader and never join a Task's work; arrangement saves still persist. Input to a
Session or shell is Work. `Commands::changes_work` classifies every command
once, exhaustively. Agent-issued commands are tool calls in the provider
transcript; `LF_PROCESS_ID` names the lf they run under and `LF_EXECUTION_ID`
hands a spawned child the id its owner row already holds.

A spawner (the Task-run loop, the Graph runner, the CI repair launcher) mints
its child's id and writes the owner row with pid and OS start at spawn. The
child writes its own end, exit and error; the spawner writes them if the child
died first. One result rule covers every owner
(`ExecutionResult::of`): no end is running, exit 0 succeeded, 130 with no error
interrupted, another exit failed, an end with no exit unknown. An AgentProcess
records no outcome, only its end and exit code. Missing or incompatible storage
leaves a recording gap; inspection and installation recovery still work. Agent
admission requires its Session row before provider launch.

### Graph outcomes and authority

A Graph run is one `lf graph <graph>`, `lf <graph>` or
`lf task run ISSUE [GRAPH]` invocation and the steps it starts as child
processes; `graph_processes` holds the run, `graph_process_steps` its steps.
Task and taskless execution share the Graph runner. A Task observes every
LfSession and Graph run associated with its checkout, plus explicitly bound work.
The shared Rust association reader includes checkout descendants at path component
boundaries and retained paths after removal. It neither follows causal ancestry
nor rewrites recorded work or usage. Session inventory exposes `task_ids` so
Desktop navigation uses the same membership as Task status.

No Graph is privileged: every one started for the Task, by naming it or in its
checkout, is equally its work. The Graph records that Task when it starts, so
reading a Task's Graphs costs the same however many other Graphs ran.
`task status`'s `work` contains all Sessions and Graph runs, including
headless and closed history. Its Task Graph record observes the most recently
launched Graph: `none`, `latest` or `finished`. Execution also reports an open
Graph in the Task's work, including one started before checkout adoption. The
`start` control is available whenever the Task can run, even when an earlier
Graph exists.

Cleanup, abandon, restore and landing repair treat all Task Graphs alike and ask
one question of one gate: does OS process evidence show any of the work's
unfinished owner records (Graph runs, steps, Session agents) live or unknown? A stopped Graph is history and blocks
nothing. A Session's turns, reserved inputs and retirement are conversation
history and never block. A process is never blocked by its own caller lineage.
A pid recorded before the machine’s last boot has exited; live or unknown
execution since then still blocks. Passive membership grants no control.
Template composition compiles into the graph, which the Graph runner holds in memory
with its cursor. Loop passes are node/iteration positions and lenses over the
step processes. They have no separate lifecycle. A past Graph keeps the graph it
launched with, whatever its YAML says now.

Each step is the command a person would type, started as a child of the Graph runner:

```text
lf --batch [options] skill <name> [message]       # agent step
lf <command> <args>                               # operation step
lf -b resume <session> --message <message>     # correction of an answer
```

Headless Session correction reserves the next captured input and claims its attachment
in one SQLite transaction after provider-custody checks. A rejected attachment claim
leaves the previous capture unchanged. Publication failure retains the reservation
and releases its attachment without reporting a provider turn.

A step learns only its own id and that it is a step (`LF_EXECUTION_ID`,
`GRAPH_STEP_ENV`). The Graph runner writes `graph_processes` (Graph name, graph
as compiled at launch, the Task it started for or none, pid, OS start, end,
exit, error) and one `graph_process_steps` row per step it started (step id,
graph node key, per-edge iterations, pid, OS start, end, exit, error). A run's
failure cause is its last failed step's error, else its own. Nothing reads the
record back to resume, and no command changes a running Graph; the control is
ending its runner.

A step's result is how its process exited. A deciding or routing step gets its
answer contract in its message, and the Graph runner reads the final answer of the
Session turn that step captured, following transport delegation
to the served command without including commands the agent ran. It accepts a JSON
value wrapped in prose or a code fence. An invalid answer is corrected by resuming the same
conversation: at most two corrective turns, then the Graph fails. After an
operation step the Graph runner looks for a landing of its checkout that is still
being watched; the Graph stops there, and neither failed. Mechanical steps create
no LfSession.
A conversation attachment grants no Graph authority.

A Graph is `current` while its Graph run has no recorded end and its pid is not
dead, `completed` when the run succeeded and `stopped` when it exited before the
last step. A killed Graph run reads `stopped` from its own pid, and the Machine
server's liveness pass records its end. Nothing restarts or resumes it. The caller,
normally the Task conversation, inspects `lf graph show ID --json` or
`lf task status`, then launches fresh work. Unknown liveness stays unknown.

### Attribution, binding and ownership

Task requires a repository and may belong to a Wave through its planning row.
Session constructors use the Task’s current membership and reject explicit disagreements.
A Session links to its Graph run through the step that captured its input
(`graph_lf_process_id`), and shares that Graph's nullable Task. `work_source`
is declared, checkout or bound when known. Historical unknown membership
stays Unknown; absence of evidence does not become Independent.

A Session's permanent assignment is write-once: null to a Task, validating any
explicit Wave against its membership. Reassignment and clearing are unavailable.
Public binding also selects that unfinished interactive Session as primary and
immediately sets its cwd to the Task's worktree; repeating binding reselects it.
Closed or headless Sessions cannot become primary. Done/landed Tasks remain
valid without reopening them. CLI states the permanent target; Desktop confirms
the exact target before writing. CLI adds no confirmation prompt. The writer
commits assignment, primary choice and cwd atomically, preserving live input and
attachment. Session cwd is the selected directory; already-running children may
keep their original directory. Process records retain their launch directory. Binding does not alter Graph membership. A selected
repository/Wave primary releases its old primary scope; ordinary Wave assignments
and conflicting permanent Task bindings remain intact.

Jack Heart's 2026-09-30 decision retains prospective usage
attribution: bind records assignment time; earlier usage retains its owner.
The history reader owns this single choice. `lf session usage --binds` compares it with
post-hoc attribution from the same history. Preserve active-turn start/assignment
evidence; unknown allocation remains unknown rather than inventing a token split.
Wave/Task moves preserve immutable historical attribution while
validating current ancestry.

`lf task run` and `lf task checkout` claim an unowned Task when they allocate its
checkout; explicit adoption and binding also claim when selecting its work or
primary conversation. General command observation never claims. Completion, abandonment and reopening write no ownership entries, so a
Done Task shows its last owner as history. The migration recorded one `recorded`
entry, person unknown, for each Task that had Started, a worktree or an admission.
A Task with any Graph has begun; absent execution evidence alone cannot prove untouched backlog.

### Readers and conversion

One indexed reader per object selects and pages identity, ancestry, command,
skill, title and status before opening payload files. No inventory scans manifests,
sidecars or live PRs to reconstruct identity. Bound conversations remain bound even
when absent from the visible planning list. Desktop panes key on LfSession identity,
so bind, rename and attachment replacement retain the surface and draft.

Current-state conversion keeps resumable native identities and selected captures.
Finished conversations, old command events and unresolved historical SQL are
not imported. Runtime readers use only the final schema and current encodings.

### Planning

Linear statuses are the owner: one In Progress Linear Project per Wave. Planned
Linear Projects express future plans; Completed ones retain history. A Task without
a graph takes up an explicitly named one, its Wave default, repository `graph:`, then builtin `code`.
There is no packet or local switch. A Wave's Project is replaced in place, preserving
active Task identity/worktree/PR/execution and refusing unrelated competing plans.
A second Machine adopts the same state through ordinary synchronization.

Adjacent APIs keep their own authority: Task PR operations own Git/GitHub;
provider routing owns credentials; Task ownership owns routing; exact process
receipts and current OS evidence own signaling. None follows from causal ancestry.

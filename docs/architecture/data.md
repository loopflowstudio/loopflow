# Data and persistence

```bash
lf session list --interactive false --json
lf session history SESSION --json
lf flow show DRIVER_PROCESS --processes --json
lf ps --json
```

A conversation, a Flow and an operating-system process answer different
questions. SQLite owns their identities and history references; large payloads
and provider-native state keep their own storage formats. This page specifies
those ownership boundaries. [Cutover status](../architecture-reference.md#cutover-status)
distinguishes this contract from the current source inventory and CLI wire format.

## Rust store API migration

The obsolete `session::Run`, `session::RunEnd`, and `sqlite::ListedRun` types
and `Store::{run,create_run,runs}` / `SqliteStore::{run,create_run,end_run,runs}`
methods are removed. This is a source-breaking change for Rust callers.
Reserve conversations with `create_session`, retain replacement inputs with
`replace_session_input`, and read conversation identity through `session` or
`sessions`. Read native evidence through `SqliteStore::session_history`.
An actual command records its Process outcome independently of agent completion.

The cutover retains current Work, account routing and resumable conversations.
Retired history stores and intermediate branch schemas have no runtime readers.

## One owner per fact

| Fact | Owner |
| --- | --- |
| Authored goals, memory, Skills, Flows and code | Repository files and Git |
| Shared planning | Linear Initiatives, Projects and Issues |
| Commits, PR heads, checks and merge | Git and GitHub |
| Actual lf command process, causal parent and observed command outcome | `processes` |
| The provider's OS process, its served Session, original parent and current attachment | AgentProcess rows in `processes` |
| Agent conversation, title, feedback and native identity | `agent_sessions` |
| Native starts, outcomes, retries and usage | LfSession history, correlated to native turn and driving Process |
| A Flow's identity, state and step results | Its driver Process and child step processes in `processes` |
| A Flow's name, launched graph and each step's node | FlowProcess: `flow_processes` and `flow_process_steps`, appended by the driver |
| A Task's Workflow: its graph, position and moves | `task_workflows`, one row per Task updated in place, and append-only `task_workflow_moves`; written by `lf task run` and `lf task move` |
| A Task's state: not ready, ready, active, done | Read from its `task_workflows` position; never stored. `tasks.abandoned_at` is the one mark beside it |
| Large captured prompts, transcripts and output | Immutable or append-only payloads referenced by their owning records |
| Credentials and provider-native conversation files | The selected provider account's native Machine |
| Current local liveness | OS process evidence matched to exact recorded PID/start identity |
| Live exclusion | Kernel-held locks and fenced database mutations |
| Selected executable and installation progress | Installation receipts |

A Machine owns its local store and payloads. Linear and GitHub remain shared truth;
a local snapshot cannot authorize a provider mutation when a fresh observation
is required. An identifier joins evidence without transferring authority.
The [checked inventory](../architecture-reference.md#complete-ownership-map)
retains exact current tables, source paths and subprocess edges in one place.

## Conversation and process history

Process records one actual lf process, including agent-issued nested commands.
Each executed Flow step has its own child lf Process. Multiple provider turns may
belong to that Process; their results remain distinct in LfSession history.
A provider may succeed before its command fails later.

LfSession identity, name and feedback survive attachment replacement. The
attached LfProcess is a nullable reference on the AgentProcess record, fenced by a
fresh token per claim. The AgentProcess has separate identity: its attached
LfProcess can die while it continues. History retains the original Process and
AgentProcess when a later attachment recovers a missed native completion. Missing command outcome, usage or process
evidence stays unknown.

A capture's `events.jsonl` holds every provider event verbatim. SQLite history
keeps what streaming increments add up to: a run of deltas is one event at the
first delta's position, a Turn keeps its last cumulative diff, and the raw
notification behind each increment stays in the file. Complete items, usage,
input and outcomes are kept in both. Sizes and the reasoning are in the
[storage footprint review](../reviews/storage-footprint.md).

A Flow is one lf process and the step processes it starts, and its ID is the
Flow process's. The driver keeps the cursor in memory and appends FlowProcess: the
Flow's name and compiled graph at launch, then each step's Process, node and
iteration counts. Nothing updates those rows, and no step reads or writes them.
A Session reaches its Flow through the step row of the Process that captured its
input. Every Flow naming a Task, or run in its checkout, is equally that Task's
work. Taskless execution uses the same driver. A step's result is how its
process exited; a deciding or routing step also answers through the Session turn
its Process captured. No generic attempt lifecycle sits between these owners.

A retry appends Session history. A killed driver leaves its Processes as history;
nothing resumes it, and its caller launches fresh work. A past Flow whose YAML
changed is drawn from the sequence its step processes recorded. Cursor movement
alone never proves an external operation happened once.

## Admission and publication

Agent admission requires a writable store before provider side effects. This
holds for headless work, interactive conversations, reviews and helpers,
even when no planning Work is registered. Optional planning enrichment is a
different dependency from required conversation persistence.

Reserve the conversation and captured-input reference, publish immutable input,
and record publication before starting the provider. Filesystem publication and
SQLite commit are separate boundaries with recoverable evidence. After a crash,
reconcile the exact saved input and launch evidence. An unpublished reservation
is not a successful launch; an absent receipt cannot prove that no provider started.

General Process observation cannot bypass installation preflight to open or migrate
an incompatible store. Observation failures remain explicit; they never justify
an agent launch without its required records. Optional stream failures preserve
missingness without inventing a terminal result.

Saved Session handoffs retain the executable, Machine and database together. A later
installation selection must not redirect an already-prepared command. Failed
launch diagnostics survive retry. Store copies preserve data, not live process
authority, and do not synchronize private writes back to the installation.

## Attribution and Started

Typed ancestry belongs to LfSession and Process. Task implies Wave;
constructors fill omitted ancestors and reject contradictions in the transaction.
Flow members share their owner's nullable Task. Historical work events retain
their recorded attribution independently of current assignment or driver.

Bind assigns an unbound conversation once, including to a done or landed Task.
Same-target assignment is idempotent; reassignment and clearing are unavailable.
The exact target is explicit and the transaction rejects competing assignment,
driver replacement or incompatible Flow membership. Prospective usage attribution
and its limits are in the
[contract](../architecture-reference.md#attribution-binding-and-started).

Actual work reservation, including first bind, sets Task Started once. Merely
recording an inspection Process does not. Later launch, chapter transfer or
conversion cannot move or erase an existing timestamp.

## Reads and cutover

Summary queries filter identity, ancestry, command, skill, title, mode and state
in SQL before loading payloads. Detail reads load only the selected capture or
transcript. Missing payloads remain visible rows with explicit missing evidence.
Passive readers acquire no driver and never launch or import.

The three migration groups create Process rows, adopt Linear Project statuses, and
cut over Session ownership directly from the released schema. Current Task
captures and review feedback survive; old command ledgers and finished
conversation archives do not. Native recovery reads the current conversation's
provider history when its selected turn needs reconciliation.

For this machine, resumable filesystem conversations are converted offline after
old writers stop and before promotion. The binary has no old-layout discovery or
import command. Rehearse on a database backup and copied captures; never point a
branch binary at the installed Machine.

## Planning and external transitions

A Chapter is the shared name of each Wave's In Progress Linear Project. Project
status owns current, planned and historical plans; there is no Chapter table,
packet or Machine-local switch. The local Project row is a synchronized projection.
Rotation converges through fresh provider facts and stable identities, preserving
started Tasks and their worktree, PR and execution. Partial rotation remains
retryable; unrelated competing current Projects remain unresolved.

No transaction spans SQLite, payload files, Git, Linear, GitHub and provider
engines. Use the smallest boundary that can prove the operation: a SQLite
transaction for related state, atomic publication for immutable input, an OS lock
for local exclusion, an exact PR head for merge, and stable provider identity for
recovering a lost response. Record uncertainty at each seam.

Released migration bytes remain immutable. Candidate promotion validates an
isolated copy and preserves current operating state before activation.
See [Machines and processes](machines.md#promote-a-new-artifact) for installation authority.

[Execution](execution.md) describes admission and recovery;
[Planning](planning.md) describes captured progression.

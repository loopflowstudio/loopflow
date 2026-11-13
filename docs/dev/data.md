# Data and persistence

```bash
lf session list --headless --json
lf session history SESSION --json
lf process show GRAPH_PROCESS --json
lf process list --json
```

A conversation, a Graph and an operating-system process answer different
questions. SQLite owns their identities and history references; large payloads
and provider-native state keep their own storage formats. This page specifies
those ownership boundaries. [Cutover status](reference-cutover.md#cutover-status)
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
| Authored goals, memory, Skills, Graphs and code | Repository files and Git |
| Saved planning | Local Task and Project rows, exchanged through a Git planning ref or a connected Linear, Asana or Notion provider |
| Commits, PR heads, checks and merge | Git and GitHub |
| Actual lf command process, causal parent and observed command outcome | `processes` |
| The provider's OS process, its served Session, original parent and current attachment | AgentProcess rows in `processes` |
| Agent conversation, title, feedback and native identity | `agent_sessions` |
| Native starts, outcomes, retries, provider transitions and usage | LfSession history, correlated to native turn and driving Process |
| Submitted creation identity and outcome | `creation_receipts`, shared by CLI and server; retained independently of the created resource |
| Desktop submissions awaiting reconciliation | Versioned immutable request payloads in the device's Work cache; outcomes come from server receipts |
| A Graph's identity, state and step results | Its Graph process and child step processes in `processes` |
| A Graph's name, launched graph, the Task it started for and each step's node | GraphProcess: `graph_processes` and `graph_process_steps`, appended by the Graph process |
| A Task's graph and the step it stands at | `task_graphs`, one row per Task updated in place; written only by `lf task run` and its child Graph process |
| A Task's state: unstarted, idle, active, moving, shipping, done | Read from its checkout, `task_graphs` step, running Graph and Auto-Ship approval; never stored. `tasks.abandoned_at` is the one mark beside it |
| Large captured prompts, transcripts and output | Immutable or append-only payloads referenced by their owning records |
| Credentials and provider-native conversation files | The selected provider account's native Machine |
| Current local liveness | OS process evidence matched to exact recorded PID/start identity |
| Live exclusion | Kernel-held locks and fenced database mutations |
| Selected executable and installation progress | Installation receipts |

A Machine owns its local store and payloads. Planning providers and GitHub hold their shared records;
a local snapshot cannot authorize a provider mutation when a fresh observation
is required. An identifier joins evidence without transferring authority.
The [checked inventory](reference.md#complete-ownership-map)
retains exact current tables, source paths and subprocess edges in one place.

## Conversation and process history

Process records one actual lf process, including agent-issued nested commands.
Each executed Graph step has its own child lf Process. Multiple provider turns may
belong to that Process; their results remain distinct in LfSession history.
A provider may succeed before its command fails later.

LfSession identity, name and feedback survive attachment replacement. The
attached LfProcess is a nullable reference on the AgentProcess record, fenced by a
fresh token per claim. The AgentProcess has separate identity: its attached
LfProcess can die while it continues. History retains the original Process and
AgentProcess when a later attachment recovers a missed native completion. Missing command outcome, usage or process
evidence stays unknown.

The provider's own transcript holds the conversation; Loopflow keeps no copy
of the stream. SQLite history keeps only Session facts: provider attempts,
accounts and sessions, how each turn ended, and silences longer than ten
minutes. Usage is the provider's native `usage` row, one per turn, updated as
receipts arrive; each turn's `completed` receipt carries its prompt context.
`session_activity` holds the latest CPU sample for Running/Stalled.

A Graph is one lf process and the step processes it starts, and its ID is the
Graph process's. The Graph process keeps the cursor in memory and appends GraphProcess: the
Graph's name, compiled graph and Task at launch, then each step's Process, node and
iteration counts. Nothing updates those rows, and no step reads or writes them.
A Session reaches its Graph through the step row of the Process that captured its
input. Every Graph started for a Task, by naming it or in its checkout, is
equally that Task's work. In a nested checkout it is the deepest Task's. A
Graph started before a Task adopted its checkout stays taskless. Taskless execution uses the same Graph runner. A step's result
is how its process exited; a deciding or routing step also answers through the Session turn
its Process captured. No generic attempt lifecycle sits between these owners.

A retry appends Session history. A killed Graph process leaves its Processes as history;
nothing resumes it, and its caller launches fresh work. A past Graph whose YAML
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

Saved Session handoffs retain the Machine and stable installed entry. Prepared
commands select the current installation when executed; private-Home handoffs
retain their explicit executable. Failed launch diagnostics survive retry.
Store copies preserve data, not live process authority, and do not synchronize
private writes back to the installation.

## Attribution and ownership

Typed ancestry belongs to LfSession and Process. Tasks require repository identity
and have optional Wave membership, recorded through the Wave's planning row.
Constructors derive ancestors and reject contradictions in the transaction.
Graph members share their owner's nullable Task. Historical work events retain
their recorded attribution independently of current assignment or attachment.

Bind assigns an unbound conversation once, including to a done or landed Task.
Same-target assignment is idempotent; reassignment and clearing are unavailable.
The exact target is explicit and the transaction rejects competing assignment,
attachment replacement or incompatible Graph membership. Prospective usage attribution
and its limits are in the
[contract](reference-models.md#attribution-binding-and-ownership).

Actual work reservation, when `lf task run` or `lf task checkout` allocates a
checkout, claims an unowned Task in its append-only `task_ownership` log. Merely
recording an inspection Process does not, nor does bind. Entries are never updated
or deleted; completion, abandonment, reopening and Wave moves write none.

## Reads and cutover

Summary queries filter identity, ancestry, command, skill, title, mode and state
in SQL before loading payloads. Detail reads load only the selected capture or
transcript. Missing payloads remain visible rows with explicit missing evidence.
Passive readers acquire no attachment and never launch or import.

The three migration groups create Process rows, adopt Linear Project statuses, and
cut over Session ownership directly from the released schema. Current Task
captures and review feedback survive; old command ledgers and finished
conversation archives do not. Native recovery reads the current conversation's
provider history when its selected turn needs reconciliation.

For this machine, resumable filesystem conversations are converted offline after
old writers stop and before promotion. The binary has no old-layout discovery or
import command. Rehearse on a database backup and copied captures; never point a
branch binary at the installed Machine.

## Query cost and the write lock

```bash
sqlite3 ~/.lf/loopflow.db ".backup /tmp/lf-plan.db"
sqlite3 /tmp/lf-plan.db "ANALYZE; EXPLAIN QUERY PLAN
  SELECT steer_id FROM session_steers WHERE session_id = 'x' AND applicable = 1;"
cargo test -p loopflow --lib store::query_plans::
```

```text
QUERY PLAN
`--SEARCH session_steers USING INDEX sqlite_autoindex_session_steers_1 (session_id=?)
```

One Machine runs hundreds of agents over about 10,000 Tasks, and every one of
them shares one SQLite write lock. A query that is fast on a fresh store can be
a full scan on that one, so plan statements against a `.backup` copy of a real
store, never the live file.

Every statement on a path that runs per hook, per server request, per Desktop
read, per Work projection or per Graph step plans as `SEARCH` or reads a fixed
small table. `SCAN` of a table that grows with use (`steers`, `session_steers`,
`steer_deliveries`, `agent_sessions`, `session_events`, `processes`, `tasks`
and their peers) is a bug. Filters an index cannot serve are the usual cause: an `OR`
across columns, a function wrapped around an indexed column, a correlated `IN`
subquery, or a per-row query inside a loop where one query would do. Fix it
with an index in the Task's migration or by rewriting the filter. Reads that
feed a screen take a `LIMIT`.

While a write transaction is open, every other writer on the Machine waits:

- Read first; open no write transaction when nothing changes.
- Keep process spawns, network calls, file IO and large reads outside it.
- Never hold one across an `await`, a request or a provider call.

`store::query_plans` traces what a hot path executes on a fixture store, then
plans each statement twice: unanalyzed, and with `sqlite_stat1` measured on a
Machine at that size. A `SCAN` of a growing table fails the test; walking a
partial index, which holds only its live subset, does not. A new hot path gets
a test there (`server::boundary` uses the same helpers). Foreign keys count:
deleting a parent row looks up its children, so a child column on a growing
table needs an index.

The Machine server keeps the statistics current: `PRAGMA analysis_limit=400;
PRAGMA optimize=0x10002` once when it starts and about once a day after, never
per connection or per command. A store that never ran a server stays
unanalyzed, which is why the guard plans both ways.

When a write still holds the lock past 3 s, the connection writes `held the
store write lock` with its call site to `logs/lf.log`, and `lf doctor` groups
those lines by binary.

## Planning and external transitions

A Wave's selected Project holds its Tasks, KRs and targets. Project rows own saved
planning, with optional provider bindings and separate delivery observations.
“Chapter” names that plan rather than another runtime object. Local changes save
before provider delivery; observed provider conflicts follow the planning writer's
rules. Replanning preserves started Tasks and their worktree, PR and execution;
competing current provider Projects require reconciliation.

No transaction spans SQLite, payload files, Git, Linear, GitHub and provider
processes. Use the smallest boundary that can prove the operation: a SQLite
transaction for related state, atomic publication for immutable input, an OS lock
for local exclusion, an exact PR head for merge, and stable provider identity for
recovering a lost response. Record uncertainty at each seam.

Released migration bytes remain immutable. Candidate promotion validates an
isolated copy and preserves current operating state before activation.
See [Machines and processes](machines.md#one-main-machine) for installation authority.

[Execution](execution.md) describes admission and recovery;
[Planning](planning.md) describes captured progression.

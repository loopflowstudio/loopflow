# Data and persistence

```bash
lf session list --interactive false --json
lf session history SESSION --json
lf flow resume FLOW_SESSION
lf ps --json
```

A conversation, a saved Flow and an operating-system process answer different
questions. SQLite owns their identities and history references; large payloads
and provider-native state keep their own storage formats. This page specifies
those ownership boundaries. [Cutover status](../architecture-reference.md#cutover-status)
distinguishes this contract from the current source inventory and CLI wire format.

## One owner per fact

| Fact | Owner |
| --- | --- |
| Authored goals, memory, Skills, Flows and code | Repository files and Git |
| Shared planning | Linear Initiatives, Projects and Issues |
| Commits, PR heads, checks and merge | Git and GitHub |
| Actual lf command process, causal parent and observed command outcome | `execs` |
| Agent conversation, title, feedback, native identity and driver | `agent_sessions` |
| Native starts, outcomes, retries and usage | AgentSession history, correlated to native turn and driving Exec |
| Captured Flow graph, cursor, return counts and claim | `flow_sessions` |
| Mechanical boundary results and consumed agent completions | FlowSession history |
| Large captured prompts, transcripts and output | Immutable or append-only payloads referenced by their owning records |
| Credentials and provider-native conversation files | The selected provider account's native Home |
| Current local liveness | OS process evidence matched to exact recorded PID/start identity |
| Live exclusion | Kernel-held locks and fenced database mutations |
| Selected executable and installation progress | Machine-install receipts |

A Home owns its local store and payloads. Linear and GitHub remain shared truth;
a local snapshot cannot authorize a provider mutation when a fresh observation
is required. An identifier joins evidence without transferring authority.
The [checked inventory](../architecture-reference.md#complete-ownership-map)
retains exact current tables, source paths and subprocess edges in one place.

## Conversation and process history

Exec records one actual lf process, including agent-issued nested commands.
Several provider turns or Flow steps may execute in that process. Their results
must not overwrite each other or become extra synthetic Execs. Command completion
belongs to the outer process; provider completion belongs to AgentSession history.
A provider may succeed before a later command operation fails.

AgentSession identity, name and feedback survive driver replacement. Its current
driver is a nullable Exec reference with a generation fence. The native engine
has separate identity and generation: a driver can die while the engine continues.
History retains the original Exec and provider generation when a later driver
recovers a missed native completion. Missing command outcome, usage or process
evidence stays unknown.

FlowSession owns one captured graph and progression. A Task selects one managed
FlowSession and may have other attributed Flows. Taskless execution uses the same
owner and driver. Every selected boundary is fenced by Flow identity, version,
claim, node and iteration tuple. Agent boundaries consume the exact successful
native history entry; mechanical boundaries record their own start and result.
No generic attempt lifecycle sits between these owners.

A retry appends history. Replacing a failed selected turn also discards its
navigation candidate; the successful successor must supply its own verdict or
route. A live turn is recovered without sending extra input. Explicit retry after
confirmed engine death preserves the earlier unknown command result and native
history. Cursor movement alone never proves an external operation happened once.

## Admission and publication

Agent admission requires a writable store before provider side effects. This
holds for headless work, interactive conversations, Asks, reviews and helpers,
even when no planning Work is registered. Optional planning enrichment is a
different dependency from required conversation persistence.

Reserve the conversation and captured-input reference, publish immutable input,
and record publication before starting the provider. Filesystem publication and
SQLite commit are separate boundaries with recoverable evidence. After a crash,
reconcile the exact saved input and launch evidence. An unpublished reservation
is not a successful launch; an absent receipt cannot prove that no engine started.

General Exec observation cannot bypass installation preflight to open or migrate
an incompatible store. Observation failures remain explicit; they never justify
an agent launch without its required records. Optional stream failures preserve
missingness without inventing a terminal result.

Saved Session handoffs retain the executable, Home and database together. A later
installation selection must not redirect an already-prepared command. Failed
launch diagnostics survive retry. Store copies preserve data, not live process
authority, and do not synchronize private writes back to the installation.

## Attribution and Started

Typed ancestry belongs to AgentSession and FlowSession. Task implies Wave;
constructors fill omitted ancestors and reject contradictions in the transaction.
Flow members share their owner's nullable Task. Historical work events retain
their recorded attribution independently of current assignment or driver.

Bind assigns an unbound conversation once, including to a done or landed Task.
Same-target assignment is idempotent; reassignment and clearing are unavailable.
The exact target is explicit and the transaction rejects competing assignment,
driver replacement or incompatible Flow membership. The current prospective
attribution assumption and its limits are in the
[contract](../architecture-reference.md#attribution-binding-and-started).

Actual work reservation, including first bind, sets Task Started once. Merely
recording an inspection Exec does not. Later launch, retry, chapter transfer or
import cannot move or erase an existing timestamp.

## Read and import

Summary queries filter identity, ancestry, command, skill, title, mode and state
in SQL before loading payloads. Detail reads load only the selected capture or
transcript. Missing payloads remain visible rows with explicit missing evidence.
Passive readers acquire no driver or Flow claim and never launch or import.

One-time import preserves interactive conversations, completed keyed Asks, Task
reviews, pending taskless reviews, headless work and command journal history.
Keep names, feedback, captures, native identities, repeated outcomes, old-ID
mappings and unknown membership. A historical provider launch does not establish
that a separate lf process existed. Do not fabricate an Exec for each old Run.

Import is idempotent and resumable after interruption. Conflicting inputs remain
unresolved with source bytes retained. Once converted, ordinary list, lookup,
rename and completion depend on the new owners; they neither scan old manifests
for identity nor recreate sidecars. Completed keyed Ask retry returns its stored
answer without launching a provider. Deletion of old owners follows preservation
proof, rather than erasing evidence to simplify the schema.

## Planning and external transitions

A Chapter is the shared name of each Wave's In Progress Linear Project. Project
status owns current, planned and historical plans; there is no Chapter table,
packet or Home-local switch. The local Project row is a synchronized projection.
Rotation converges through fresh provider facts and stable identities, preserving
started Tasks and their worktree, PR and FlowSession. Partial rotation remains
retryable; unrelated competing current Projects remain unresolved.

No transaction spans SQLite, payload files, Git, Linear, GitHub and provider
engines. Use the smallest boundary that can prove the operation: a SQLite
transaction for related state, atomic publication for immutable input, an OS lock
for local exclusion, an exact PR head for merge, and stable provider identity for
recovering a lost response. Record uncertainty at each seam.

Released migration bytes remain immutable. Forward migrations and offline import
preserve historical facts; removing a product object does not permit losing its
evidence. Candidate promotion validates an isolated copy before activation.
See [Homes and processes](homes.md#promote-a-new-artifact) for installation authority.

[Execution](execution.md) describes admission and recovery;
[Planning](planning.md) describes captured progression.

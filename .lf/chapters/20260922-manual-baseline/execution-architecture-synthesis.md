# From a remote execution service to sovereign Homes

## Scope

No chapter-start snapshot exists. This report treats the execution architecture
visible at the beginning of summer 2026 as “before” and the repository state on
22 September as “after.” It is a synthesis of two independent narratives: one
written in this Run and one produced by a separately configured Claude Run. Both
were completed before applying the decentralization thesis below.

## The architectural hinge

The most concrete account of decentralization is a change in execution topology.

At the beginning of summer, a remote client—especially the Mac app—spoke HTTP to
`lfd`. `lfd` was not only keeping services alive; its API sat on the path by
which clients read and acted on runtime state. Product memory records Concerto
reaching remote `lfd` over HTTPS through Tailscale on 30 June. The later deletion
receipt is revealing: `LocalWaveService` and `WaveServiceProtocol` comprised
roughly 1,500 lines, with about 22 consumers rerouted when the path was removed.

By the end of the chapter, the remote path was command-shaped instead:

```text
before
client -> HTTP -> lfd -> machine state and execution

after
client -> lf ssh -> target Home's lf -> that Home's files, store,
                                        credentials, processes, and providers
```

`lf ssh` is transport, not a second API. The target machine runs the same `lf`
implementation used locally, verifies its own Home identity, resolves its own
state, and returns the result. The Mac app likewise reads shared `lf --json`
projections through `RegistryQuery` instead of maintaining a separate HTTP
lifecycle client. `lfd` remains useful, but smaller in meaning: it keeps one
Home's services, receives webhooks, reconciles Wave listeners, and claims
landing work. It is no longer the universal execution or read authority.

That is the chapter's main decentralization move. It replaces one privileged
remote service surface with complete local execution on each Home.

## The boundary that made it possible

The topology would not work if “the Work” were whatever happened to be alive
inside `lfd`. A durable Wave, Project, or Task must survive transport, daemon
restart, provider replacement, and process death. Conversely, a process launched
for that Work must not inherit authority over the Work merely because it knows
its id.

The summer therefore also clarified the Work/execution handoff:

- Work preserves purpose, durable input, status, and the controller playhead.
- A controller loads current Work facts, chooses one boundary, executes it,
  records one monotonic transition, and can disappear.
- A Run is Home-local evidence for one mediated provider launch. Work
  attribution explains why it launched; it does not reserve or mutate Work.
- A provider-native Session preserves conversation continuity. It is not the
  executor and not the Work.
- The process that directly spawns a child owns that child handle. A Run id,
  Work id, PID, tmux name, parent Run, or telemetry row does not manufacture
  signal authority.
- The OS supplies current liveness facts. Placement only says which Home owns
  the next execution decision.

This boundary was reached through a reversal. On 18 July, LOO-196 and PR #1099
deleted Project and Task Session controllers and made Run the sole shared
executor. That centralization was necessary: several Session, runner, lease,
and body types had been claiming the same lifecycle. Dogfooding then showed Run
was still being asked to mean too much. By September, Run had been narrowed to
evidence and causality, while control, continuity, purpose, and liveness had
returned to their real owners.

Session followed the same pattern. It was deleted as a Loopflow executor, then
returned by 30 August with a smaller, accurate meaning: a provider-native
conversation or unresolved human boundary. tmux became a PTY cradle; the Mac app
became a presenter; neither owned the Session.

## Why the rest of the chapter looks like hardening

Once each Home executes locally, guarantees previously hidden inside a single
service have to become explicit.

**Identity cannot be recovered from location.** Worktree and branch names had
encoded ancestry; environment variables could make a promoted process attach as
the wrong role. Stable identity was separated from directory, branch, route, and
process environment. Main agents moved into their own worktrees so a resident
could not contaminate canonical main and block later Task execution.

**Recorded state cannot stand in for current truth.** Status replayed an old
credential failure while current runners were healthy; dead invocations remained
“running”; containment was shown as provider progress. The correction was to
label historical facts as historical and consult current OS evidence for
liveness. An unterminated Run now means only “no terminal receipt,” not “alive.”

**Observation cannot gate execution.** Missing thread-local Run context, SQLite
trace contention, and incomplete journal or Invocation receipts killed or
stranded healthy Task work. August changes made Run context explicit, trace
capture nonfatal, and process truth—not telemetry completeness—the basis for
Task recovery and promotion.

**Remote scope cannot stay ambient.** Run records, credentials, processes, and
OS locks are Home-local. A remote reader executes on the named Home. There is no
implicit fan-out or central Run database. “Unavailable,” “stopped,” and “unknown”
must remain distinct.

**Recovery must rebuild from durable boundaries.** Missing flows now settle
instead of retrying indefinitely. Controllers rebuild from Work facts rather
than a surviving Session. Exact PR heads and narrow locks own delivery races;
broad runtime ownership does not.

Read this way, the August work was not a collection of unrelated lifecycle bugs.
It was the work of making the local-command topology true under restart,
concurrency, missing telemetry, provider replacement, remote placement, and
partial failure.

## Where the redesign stopped

The execution model is more coherent than its observation layer.

The baseline review found settled records without context, replayable launch
contracts, token evidence, or cost, and no unattended replay cohort. A prior
ledger outage silently lost 29.2 hours of writes. The product still observed
cases where Work, Run, process, Git, and presentation state disagreed. Shared
`lfd.db` migration collisions also show that one remaining multi-owner store can
still reproduce the blast radius the execution redesign removed elsewhere.

There is a deliberate usability cost too. The system sometimes cannot safely
control what it can observe. A visible PID does not prove signal authority; an
unterminated Run does not prove a live process; an unreachable Home cannot be
silently called stopped. The architecture accepts `unknown` rather than invent
control.

## Consequence for Product and Intelligence

The topology is an Infrastructure story in the architectural sense, but not in
the Work-attribution sense.

Product carried most of the execution redesign's semantic core. Its
`loopflow-api` Tasks include Session-to-Run convergence, cross-provider Run
recovery, lifecycle capability boundaries, explicit Run context across async
runners, telemetry that cannot kill healthy Work, watched landing, and durable
human checkpoints. Product memory also owns the Mac HTTP-client deletion and
the move to `RegistryQuery`/`lf --json`. Product did not merely project an
Infrastructure substrate; it built much of that substrate.

That work displaced Product's named surface outcomes. The major Sessions and
fleet-loading Tasks remain open. One Mac Surface KR holds—the app did not create
a second Work runtime or company transcript—but the other six fail or remain
unknown. PM editing, recovery controls, honest liveness, iOS, and coherent fleet
status are still incomplete. The Wave spent much of August making the runtime
safe enough to expose rather than finishing the exposure.

Infrastructure contributed the Home identity and placement pieces, startup and
wake behavior, worktree isolation, architecture checking, promotion, releases,
credentials, and cron continuity. Its receipt record is more operational than
the label “Infrastructure execution redesign” suggests. It also contains a
material amount of trace and ledger work that fits Intelligence's stated
mandate, including concurrency, lineage, capture survival, usage parsers, and
telemetry continuity.

Intelligence was registered as current Work only on 19 August, though its memory
predates that registration. It adapted the contracts late but coherently:
immutable pre-launch execution contracts, strict replay preflight, unattended
replay, and real-ledger audit survival all match the new authority model. It did
not adapt the operating evidence. Every Trace KR is false; real-ledger capture
audits reported 1,767 decode failures; scheduled telemetry failed; settled Runs
still lack context, contracts, tokens, or cost; and the activity projection has
no delivery receipts for several Linear-complete Trace Tasks. The observation
layer cannot prove some of its own work happened.

The Wave map therefore lagged the architecture. Product became the home of
runtime semantics, Infrastructure the home of operations and several leaked
telemetry concerns, and Intelligence the late owner of the evidence layer both
had already been changing.

The next Intelligence chapter should rebuild monitoring on top of the execution
topology rather than beside it.

The useful trace is a non-authoritative join:

```text
Work boundary
  -> owning Home and selected runtime artifact
  -> immutable Run launch contract
  -> provider account, attempt, and native Session
  -> owned or merely observed OS process
  -> events, usage, and terminal proof
  -> resulting Work transition
  -> delivery outcome
```

Every edge needs a source owner, stable identifier, timestamp, freshness, and
explicit missingness. A cross-Home index may help users, but it must be
rebuildable and must never become the fact that decides whether a process is
alive or whether Work may advance.

## Judgment

The decentralization thesis holds when stated precisely: Loopflow moved from
remote clients calling a lifecycle-shaped daemon API to complete `lf` execution
on the Home that owns the relevant processes and evidence. The Work/execution
split defined what could safely cross that boundary, and most later runtime work
made the split operationally honest.

“Decentralization” becomes misleading only when it suggests that every component
became independent or that centralization was uniformly bad. The redesign also
created stronger centers: one `lf` implementation, one Run-record contract, one
provider-native Session, one direct process owner, one exact PR head, one source
for each fact. The achievement was not removing centers. It was replacing a
general-purpose execution center with several narrow authorities whose claims do
not silently transfer to one another.

## Independent-reading comparison

Both independent narratives organized the summer around separating identity,
continuity, control, and observation. Claude called decentralization “secondary”
because the first decisive July move centralized execution in Run. The present
synthesis differs in emphasis: it treats the `lfd` HTTP API to `lf`-over-SSH
transition as the topological hinge and the authority separation as the work
required to complete it. The underlying observations do not conflict. The exact
date of the HTTP-to-SSH cut is not durably retained, so it is described as a
summer transition rather than attributed to one invented keystone commit.

## Sources

- `scratch/execution-architecture-evolution.md` — first independent narrative.
- `scratch/research-execution-narrative-claude-97b2ab32.md` — Claude's
  independent narrative.
- `wave/product/MEMORY.md` — June remote-`lfd` path, Work/Run vocabulary,
  RegistryQuery convergence, Session revision.
- `wave/infrastructure/MEMORY.md` — Home lifecycle, worktree identity, control
  and recovery boundaries.
- `wave/intelligence/MEMORY.md` — ledger failures and trace constraints.
- `docs/architecture/{execution,planning,homes,data}.md` — current contracts.
- `lf activity` and `lf status` — dated merge and Task evidence.

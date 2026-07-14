# Wave → Project → Task: architecture and ship review

## Objective

Make three domain loops the whole product model:

```text
Human ↔ Wave → Project Session → Task Session → PR to main
                  └────────────→ Task Session → PR to main
       └───────────────────────→ Task Session → PR to main
```

- Humans create and talk to Waves.
- Every Project belongs to exactly one Wave.
- Every Task belongs to exactly one Project before execution.
- A Wave may supervise a Task directly for a small change, but that Task still
  has a Linear Project. “Direct” skips a Project Session; it never creates an
  orphan Task.
- Waves and Project Sessions coordinate from clean canonical `main`. They own
  no delivery branch, worktree, or PR.
- A Task Session is the only domain runtime that owns a worktree. Its Linear
  issue exists first.
- Provider processes and any parallel hands are implementation details inside
  the owning domain session. A Worker/Exec never independently acquires a
  worktree or becomes a fourth planning noun.

The hierarchy is about responsibility, not size. Wave chooses bets and remains
the human surface. Project pursues KRs across Tasks. Task ships one concrete
change through merge or explicit abandonment.

## User behavior

The record-first path is formal:

```bash
lf project run <linear-project-id>
lf task run INF-123
```

Free text remains an API, not an anonymous execution path:

```bash
lf project start "make first-run onboarding obvious" --wave infrastructure
lf task start "add hello-world" --project <linear-project-id>
```

`start` creates the Linear record, refreshes the owning Wave’s atomic PM
snapshot, and then invokes the same `run` lifecycle. If creation fails, no
session or worktree appears. If creation commits but refresh cannot confirm it,
the durable provider identity must be retained and reconciled; a retry must not
create a duplicate issue.

The human normally uses Wave Chat. The Wave performs those operations, stays
available while children work, and exposes the same hierarchy and controls in
the Mac app:

```text
Wave
├── Project: first-run onboarding
│   ├── Task INF-123: command behavior
│   └── Task INF-124: docs and tests
└── Project: release reliability
```

One composer always talks to the Wave. Child cards provide inspect, steer,
follow-up, interrupt, resume, wait, decide, attach, and abandon operations
without silently moving the human into a child transcript.

## The three flows

Each tier uses the same semantic rhythm:

```text
clarify → pursue → mutate → deterministic controller decision
```

The skills do judgment and work. They do not write a loop bit.

### Wave

- `wave_clarify` keeps the Wave objective and planning context computable.
- `wave_pursue` selects a Project, starts or steers its Project Session, or
  starts one already-projected Task directly.
- `wave_mutate` integrates the result into the durable operating context.

One wake runs the complete flow and then idles. A human message, typed child
observation, cron, or coarse heartbeat wakes it again. The Wave has no terminal
state and must not buy provider turns merely because it remains alive.

### Project

- `project_clarify` makes the captured Linear Project’s KR set measurable.
- `project_pursue` creates/selects Tasks and supervises their sessions.
- `project_mutate` evaluates current PM, Task, decision, and delivery evidence.

After the full flow, the Project controller chooses:

- every KR holds → `Completed`;
- Tasks are active and only child progress can change the answer → `Waiting`,
  with no process;
- a decision or resumable child needs judgment → remain actionable;
- observable state changed and open KRs remain → another iteration;
- no state changed and open KRs remain → `Blocked`, never busy-spin.

The Project Session is durable pursuit state, not another Wave. It has a
resumable provider transcript and process generations, but no chat address,
memory, cadence, worktree, branch, or PR.

### Task

- `task_clarify` turns the Linear issue and delegation into a computable change.
- `task_pursue` edits, tests, commits, and updates the one Task PR.
- `task_mutate` evaluates delivery and review evidence.

After the full flow, the Task controller repeats only when work is actionable,
waits through review and CI without losing its session, and ends only when the
PR is merged or the Task is explicitly abandoned. “A skill finished” and “the
Task finished” are intentionally different facts.

## Child steerability

Steerability is a durable control protocol around provider transcripts, not
terminal input.

### Stable address and process generations

One Linear Project or issue resolves to one durable Project/Task Session.
Provider session ids and tmux processes may change or stop; the domain Session
id, command history, event ledger, and—only for Tasks—worktree stay fixed.
Every process generation claims unresolved commands. A dead generation loses
its claims; accepted commands never return to pending.

### Three instruction intents

```text
follow-up  preserve the active turn; deliver exactly once as the next turn
steer      change direction now; inject live or interrupt-and-resume
interrupt  stop this turn; optionally supersede pending input with replacement
```

All return a durable `cc_…` receipt with persisted/claimed/accepted/failed/
superseded state, actual effect, generation, timestamp, and error. Receipt
waiting is explicit and recoverable.

Codex supports live injection. Claude and OpenCode currently implement steer
as interrupt-and-resume in the same provider transcript. Orchestration branches
on the Harness capability, never on provider names.

### Directive incorporation

Provider acceptance is not evidence that the child acted on the instruction.
Versioned directives remain pending until a later child result acknowledges
incorporation. Replacements supersede older unincorporated versions. Wave Chat
shows both transport state and incorporation state.

### Atomic turn boundary

At a Project/Task turn boundary, one SQLite transaction either claims queued
commands or marks the generation inactive. A racing command therefore reaches
the current generation or reserves and launches the next one; it cannot remain
stranded in a waiting Session because a polling window closed.

### Typed observations

Task and Project ledgers are authoritative. An observation outbox links each
consequential event to its immediate supervisor and survives stopped processes.
Project consumption and acknowledgement are transactional; the Wave journal
append is idempotent before acknowledgement. Raw child tool chatter stays in
the child transcript.

The root Wave retains visibility into descendant direction, failure, blocker,
delivery, and completion events. Decisions preserve the responsibility chain:

```text
Wave → Task:             Task decision → Wave
Wave → Project → Task:  Task decision → Project
                        Project answers or emits a Project decision → Wave
```

The owning Wave may override any descendant. A foreign Wave or unrelated
Project is refused before command persistence. An unattributed local human is
the explicit escape hatch, never an error fallback.

### The remaining exact-once limit

A process can die after a provider accepts input but before SQLite records
acceptance. Loopflow can guarantee persistence, claiming, boundary settlement,
and deterministic recovery around that window; it cannot prove provider-side
exactly-once without provider idempotency keys or reconciliation evidence.
Keep that limit visible rather than hiding it under more local state.

## Major technical changes in this branch

1. **Linear-owned planning.** Linear Initiative → Project → Issue is the only
   roadmap hierarchy. SQLite holds one atomic `PmShowResult` read snapshot per
   Wave; lifecycle code consumes it through `lf pm` rather than querying Linear
   or reading Project markdown.
2. **Durable Task Sessions.** A Task launch receipt binds Linear UUID and human
   identifier, Project, Wave, snapshot time, provider, immutable sibling
   worktree/base/branch, process generations, PR, and PM writeback state.
3. **Durable Project Sessions.** Project pursuit gains one resumable provider
   thread, process generations, iteration/fingerprint state, supervised Tasks,
   decisions, and observation cursor without gaining a worktree or Wave
   identity.
4. **One child control core.** Project and Task commands share `child_commands`,
   `cc_` receipts, versioned directives, atomic boundary settlement, decisions,
   and failure/supersession rules. Domain events remain distinct.
5. **Typed supervision.** The observation outbox carries Task → Project,
   Task → Wave, and Project → Wave events durably and idempotently.
6. **One hierarchy in status and Wave Chat.** CLI JSON and Swift project native
   Wave → Project → Task identity instead of presenting children as generic
   Runs. Wave Chat has one composer, a Project/Task work map, structured
   activity cards, decision options, and child controls.
7. **One clean control plane.** Wave and Project processes run from canonical
   `main`, fail on dirty state, and never ship code. Only Task placement creates
   a worktree; every Task PR targets `main`.
8. **One flow semantics.** Wave/Project/Task all run clarify → pursue → mutate;
   controllers own repeat/wait/block/complete. The Wave yields after one full
   wake. Its typed `(harness, provider_session_id)` survives resident restart.
9. **Competing execution removed.** Public generic loops, detached-loop routes,
   `lfq`, generic exec proxying, stack/queue/combine delivery, rotation, and
   local Project mirrors are gone. One-shot skills remain utilities, not a
   fourth durable loop.
10. **Large-branch recovery made local.** `lf rebase --manual` preserves the
    sequencer for inline edit/continue/abort and never pushes. Automatic
    conflict fallback no longer embeds every changed file into one argv and
    overflow the OS limit.

## How the change became this large

The expansion was a sequence of local answers that eventually forced one
global model:

1. The original Wave resident was durable and steerable; the generic placed
   loop was isolated but only observable at pass boundaries.
2. Worktree redesign made Wave homes permanent and removed land rotation. That
   exposed the mismatch between durable minds and ephemeral file-writing hands.
3. Linear’s native Initiative/Project/Issue hierarchy made anonymous work and
   local Project mirrors increasingly incoherent.
4. Task Session work joined Linear identity, immutable placement, provider
   history, commands/events, PR lifetime, and merge writeback into one owner.
5. Benchmarking Codex, Claude, and OpenCode showed that “send” was not one
   operation. Follow-up, immediate steer, replacement, receipts, decisions,
   resume, and automatic parent observation needed explicit semantics.
6. Removing the generic loop also removed repeated Project judgment. Project
   Sessions were introduced to pursue KRs across several Task Sessions without
   promoting every bet into a Wave.
7. The terminal lifecycle made the Mac app’s generic Run projection visibly
   wrong. Wave Chat had to show the native hierarchy and keep the human on the
   Wave conversation.
8. Once all three tiers were explicit, their flow semantics could stop relying
   on LM-authored loop bits: each runs clarify/pursue/mutate and a deterministic
   domain controller decides what follows.
9. The gate and rebase then exposed operational seams—provider-thread recovery,
   nested decision routing, trace/session composition, and large-argv rebase
   fallback—which were fixed rather than documented away.

The breadth is therefore not one feature accreting helpers. It is the deletion
of several mutually inconsistent answers to identity, repetition, placement,
steering, and UI ownership.

## Architecture review

### What now has one owner

| Truth | Owner |
|---|---|
| Wave objective and Initiative binding | `wave/<wave>/GOAL.md` |
| Wave memory and human conversation | Wave server/journal |
| Project definition, KRs, Tasks | Linear |
| Local planning reads | atomic Wave PM snapshot |
| Project pursuit runtime | Project Session + Project event ledger |
| Task delivery runtime | Task Session + Task event ledger + worktree |
| Child controls and receipts | `child_commands` |
| Supervisor delivery | observation outbox |
| Provider conversation continuity | typed provider session reference |
| Code delivery | one Task PR to `main` |

### Clear next reductions

1. Documentation still contains the old “Wave advances continuously” wording
   and an outdated resident-context wire shape. Correct it to one full flow per
   wake and include the typed provider session.
2. `flowloop/wave.rs` is now the Wave domain runner after generic flowloop
   deletion. Move or rename it only if that materially simplifies module
   boundaries; do not churn names as a cosmetic exercise.
3. `WorkerRecord`/`workers` still exists in Wave health and journal language.
   Decide whether it is low-level process telemetry inside Tasks or a leftover
   product noun. It must not imply that Waves directly dispatch independent
   worktree-owning Workers.
4. Project and Task runners deliberately duplicate domain policy, but their
   provider command pumps may still contain mechanical duplication. Reduce only
   the truly identical control core; do not introduce a generic session
   framework or factory trait.
5. `lfdb::{mod,sqlite}` now owns a very large persistence surface. Grouping by
   domain may improve legibility, but SQLite transaction boundaries must remain
   visible and shared child-control operations must stay atomic.

### Questions still open

- Does real multi-Task dogfood justify a persistent Project provider transcript,
  or can some Projects be pursued entirely by the Wave with Project Session as
  durable state but no independent LM conversation?
- Which Task decisions should map from native provider approvals, and which
  remain explicit domain decisions? Auto-approval is not a complete product
  policy.
- Should standing frontier Projects wait indefinitely when Tasks are quiet, or
  block on unchanged state like milestone Projects? Current behavior blocks.
- What evidence should close the provider acceptance window: provider
  idempotency keys, transcript reconciliation, or an explicitly at-least-once
  contract?
- How should future dependent Tasks or multi-Task integration preserve “one
  Task, one worktree, one PR to main” without restoring stacks and queues?
- What is the smallest Wave Chat inspector that makes receipt state,
  incorporation, decision lineage, provider transcript, worktree, and PR
  understandable without turning the Wave UI into three separate consoles?

## Export map

The branch should remain coherent through live dogfood. If individual choices
need to land separately, the safest seams are dependency-ordered:

1. large-branch local rebase recovery;
2. Wave one-wake flow boundary and typed provider-session recovery;
3. nested decision routing and provider-neutral child conformance tests;
4. native status/DTO hierarchy;
5. Wave Chat hierarchy and child activity UI;
6. Project Session runtime and Task supervision;
7. removal of the competing generic execution/delivery surfaces.

Do not export persistence migrations, CLI nouns, and UI DTOs independently
when doing so creates a temporary second lifecycle. Extraction is useful only
when each landed slice leaves one truthful model.

## Readiness and remaining gates

The rebased branch passes:

- Python: 52 tests;
- Rust format/clippy and 1,344 tests, 3 intentionally skipped;
- website: 59 tests, 3 intentional skips;
- Swift: 292 package tests plus 5 XCTest boundary tests;
- CLI/API end-to-end smoke;
- signed macOS `build-for-testing`.

The deterministic implementation is ready to ship. One side-effecting manual
gate remains and must not be simulated:

```text
Wave creates/selects one small Linear Project
→ Project Session starts two Linear Tasks
→ Task requests a decision; Project answers and escalates one to Wave
→ Wave steers the Project while both Tasks run
→ Project sleeps; typed Task events wake it once
→ both Tasks open PRs, resume through review, merge, and reconcile Linear
→ Project verifies KRs and emits one typed completion to Wave Chat
```

Record the Linear/Project/Task/Session/command/decision/provider/PR ids and
sleep/wake evidence. This creates external records, spends provider tokens,
opens PRs, and merges code, so it requires an explicit human-authorized run.

Until that gate runs, keep the provider acceptance window and live UX quality
as evidence gaps—not reasons to invent more architecture.

## Current reduction: persistence follows the product boundary

Project and Task events now nudge the same durable outbox drain. The live Wave
receives both immediately; a stopped Wave catches up on serve. No child may
select a payload or write the Wave journal.

The persistence facade still carries an older form of speculative
flexibility. `WaveStateStore`, `RepoStore`, `ExecutionStore`,
`ControlSessionStore`, `TokenStore`, and `StoreAdmin` each have exactly one
implementation: `Store`. Nothing consumes their trait objects. Every public
inherent method calls its own trait method, which then copies owned arguments
and calls SQLite. This doubles the API surface and hides the actual owner
without providing substitution, isolation, or a test seam.

The single-implementation traits are now gone. `Store` is the one asynchronous
registry API, and its methods cross the blocking SQLite boundary directly.

Use the same ownership boundary to split the oversized implementation:

- `lfdb/child_sessions.rs` owns the async Project/Task session, command,
  directive, event, and outbox facade;
- `lfdb/sqlite/child_sessions.rs` owns their synchronous queries, row maps,
  and transactions;
- the parent modules retain Wave/run/repo/token/trace persistence;
- no generic child store trait or backend interface is introduced;
- transaction bodies and SQL remain byte-for-byte moves.

This makes the central persistence files smaller without pretending Project
and Task are the same domain. They share a durable command/outbox mechanism;
their event and session types remain distinct.

## Current reduction: one child-command envelope

The persistence split exposed one remaining false distinction. SQLite already
stores Project and Task control in one `child_commands` table, command ids are
already shared `cc_...` ids, directives already target `ChildRef`, and the
provider-control loop already handles both domains. Rust nevertheless rebuilds
the same command envelope twice as `ProjectCommand` and `TaskCommand`, then
duplicates insert, claim, supersede, receipt, accept, and failure APIs for each.
The public ops layer has consequently drifted: Project follow-ups wait for a
two-second acceptance window while Task follow-ups return after durable
persistence.

Make the storage model truthful:

- one concrete `ChildCommand { target: ChildRef, ... }` crosses both runners;
- one set of persistence operations owns creation, supersession, claiming,
  receipt reads, acceptance, and failure;
- Project and Task keep distinct sessions, statuses, events, boundary
  transitions, launch policy, and user-facing commands;
- no public generic session noun, factory trait, or extensible target registry
  is introduced;
- follow-up, steer, interrupt, resume, decide, and abandon keep one intent at
  both control edges.

This is shared mechanism, not a fourth product concept. A human still says
Project or Task; `ChildCommand` exists only where both nouns genuinely use the
same durable protocol.

## Current reduction: one control submission path

The envelope is shared, but the public Project and Task operations still each
implement its whole lifecycle: replacement directives, decision idempotency,
supersession events, inactive relaunch, short receipt waits, incorporation
waits, and result projection. That is enough duplication to change behavior:
Project follow-ups wait for provider acceptance while Task follow-ups return as
soon as the instruction is durable, and an inactive Task can be abandoned
without spending a provider turn while an inactive Project is relaunched.

Collapse the common protocol behind one private, concrete two-variant target:

- Project and Task wrappers still resolve their own public identity, ownership,
  liveness, and source attribution;
- the shared path persists the command/directive, emits the right domain event,
  relaunches the right session kind, and returns one receipt shape;
- inactive abandonment settles locally for either kind;
- follow-up always returns after durable persistence; steering and replacement
  retain their short convenience wait and durable receipt id;
- each domain still owns its process launcher, status enum, terminal meaning,
  and event vocabulary.

Do not expose `child` as a CLI noun. It is the protocol between the three
product nouns, not a fourth thing a human manages.

## Current reduction: remove the app's phantom Run history

The Mac now presents the product hierarchy directly: Project/Task state sits
beside Wave Chat, child observations render in the conversation, and selecting
an item prepares a message to the Wave. Under that surface, `RepoState` still
maintains a per-Wave `RunStore` populated from `lf status`. No production view
reads it. The iOS detail view and terminal focusing path still request it, tests
still defend its 50-row cache, and every five-second attention refresh spends
time constructing rich `Run` objects only to discard them.

Delete that competing projection:

- `lf status` still decodes its required `runs` wire field, but the app does not
  reinterpret those rows as a second work hierarchy;
- Wave Chat uses its streamed playhead for the active Wave turn;
- the work map owns Project/Task runtime and delivery state;
- `lf runs` remains the separate machine activity/usage ledger where a generic
  execution timeline is actually the product;
- `RepoState` keeps attention and terminal sessions, but no invisible Run
  cache or dead load/clear lifecycle.

That first slice leaves the internal `Run` type in place long enough to judge
its remaining uses without the cache making them look live. The following pass
then removes it from the Wave model itself.

## Current reduction: make Wave turn state belong to Wave Chat

After removing the cache, the remaining app `Run` model has no live producer.
The registry path creates Waves from `lf ls`, which carries rolled-up Wave
status but no `active_run`, `flow_steps`, or recent skill executions. The Mac
gets the real active turn, step provenance, retries, and loop state from Wave
Chat's replayable SSE playhead. Project/Task status and PR delivery come from
the work map. Only the retired HTTP parser, mock data, and tests still populate
`Wave.activeRun` and `WaveViewModel.recentSteps`.

Remove the shadow Wave lifecycle:

- a Wave model carries identity, objective, status, and authored operating
  context—not a generic worker Run or shipping PR;
- Wave Chat owns current-turn and flow-step presentation;
- the work map owns Project/Task execution and delivery presentation;
- the telemetry ledger owns historical generic process activity;
- the sidebar shows durable Wave status and its authored tagline, without an
  activity timestamp no production path can supply.

This deletes the app's `Run`, `PullRequest`, and `StepRun` types. The required
`runs` field in the `lf status` wire snapshot remains decoded but deliberately
unprojected until the Rust status contract can drop it separately.

## Current reduction: remove the unshippable mobile shell

The Mac speaks to the Wave through its replayable local chat endpoint. The iOS
app still targeted the retired generic agent-session HTTP API: its remote
handshake returned no repositories, it had no registry query, and output
streaming ended immediately. Setup could never reach a real Wave. Its `run-ios`
helper made this harder to see by asking Xcode to build the macOS scheme for an
iOS destination.

Remove the iOS app target, shell, simulator helper, and stage verifier. Keep the
shared library iOS-compatible so a future mobile product can reuse the typed
models. Reintroduce mobile only with an explicit remote Wave discovery/chat
transport and the same child-observation contract as Mac; do not revive generic
exec/session HTTP to make a screen connect.

## Current reduction: process sessions are not product sessions

Removing the broken mobile chat exposed an entire second app architecture that
no production view could reach: `SessionState` created generic provider chats,
`TerminalWorkspaceStore` projected generic lfd processes into a selectable
workspace, and `MultiplexerStore` persisted arbitrary terminal/markdown/diff
pane trees. Their only remaining server methods either returned an empty list
or threw “retired API”; tests and previews kept the graph looking alive.

Delete that application layer:

- Wave Chat is the only human conversation state;
- Project and Task Sessions are the only child runtime state shown in the work
  map;
- terminal/process rows remain a low-level lfd wire DTO and telemetry concern,
  not a fourth app hierarchy;
- `RepoState` discovers Waves, attention, worktrees, and authored context but
  no longer creates, attaches, starts, cancels, focuses, or multiplexes generic
  sessions;
- tests for unreachable stores and controls go with their implementations.

Keep the Swift terminal `Session` DTO mirror because Rust and Python still
publish that lfd contract. The important boundary is behavioral: a technical
process record does not imply a user-facing Session lifecycle.

## Current reduction: remove the unreachable command shell

The generic session hierarchy had a matching shell: a keyboard router,
command palette, help overlay, area typeahead, and multiplexer shortcuts. The
app injected the router and posted a ⌘K notification, but no production view
read either one. The views existed only in previews and the shortcuts existed
only in tests; several commands targeted the multiplexer just deleted.

Delete the whole disconnected surface. A command or shortcut belongs in the
app only when it invokes a live Wave, Project, or Task behavior. Reintroducing
a palette later should start from the current product actions—select a Wave,
message it, inspect its work map, and prepare a child directive—not from the
retired generic terminal/workspace model.

## Current reduction: delete terminal ownership without deleting terminals

The app also retained unused launchers for eight external terminal/IDE apps,
per-workspace tmux creation and shutdown, app-icon lookup, focus notifications,
and a window accessor. None had a caller after the workspace and command-shell
removals. Keeping them would imply that the app still owns generic execution
placement.

Delete those helpers and their preference/test surface. Keep the reachable
Ghostty diagnostic window, GUI process-environment repair, and the Wave's own
detached tmux lifecycle. A terminal remains a display or debugging tool; Wave,
Project, and Task lifecycles decide what work exists and where it runs.

## Current reduction: one app orchestrator

With mobile gone, `RepoState` has no production owner. The Mac has already
moved to `PortfolioRepoState`; the thousand-line predecessor survives only so
`WavesView` can call its static UI-test-mode parser and one test can instantiate
it. Its connection, Wave mutation, trigger, attention, worktree, and catalog
paths therefore describe an application that no longer runs.

Move the test-mode parser into a small Mac type, keep `SharedDaemon` as the
reachable bundled-daemon owner, and delete `RepoState`. The app now has one
orchestrator per repository and one source for its Wave/Project/Task view.

Delete the caches that only `RepoState` owned as part of the same architectural
collapse: `WaveStore`, `AttentionStore`, `WorktreeStore`, and `OutputBuffer`.
The live Mac paths already hold their small projections directly. Local
notifications also had no remaining producer, so startup no longer asks for a
permission the product cannot use.

The same pass removes the local roadmap parser and `WaveContent` projection.
Linear owns Projects, KRs, and Tasks; the app already reads that hierarchy from
`lf status`. The Wave row's subtitle now comes from the authored Wave objective
that `PortfolioRepoState` actually loads, rather than a permanently empty
legacy README/roadmap cache.

The next seam is the retired `WaveService` façade. It has no successful live
operation: registry reads moved to `lf --json`, Wave Chat owns its own stream,
remote mutations throw, catalog/worktree reads return empty, and its last auth
store has no rendered consumer. Delete the façade and the auth/catalog/worktree
types that only made those dead endpoints appear supported. Keep wire fixtures
decoded by their actual DTOs rather than routing them through a service parser.

The same ownership test makes the app's bundled/remote connection mode
untenable. Every durable read is a local `lf` subprocess and every conversation
uses a locally discovered Wave endpoint. The private bundled `lfd` writes a
different database and has no consumer; “remote mode” only suppresses that
unused daemon without making queries or chat remote. Remove both modes, their
HTTP/TLS/token machinery, and the private daemon. The app now starts no
machine-wide service: it queries the local registry and launches `lf serve`
only for the Wave the human opens.

That also retires the embedded-terminal build boundary. Ghostty remains only a
debug window after Wave Chat becomes the product surface, yet every Swift build
still downloads its binary framework and links terminal-only dependencies.
Remove the debug terminal, Ghostty package, no-op beta toggle, unused CLI
installer, and the Flow/Skill/generic Session models that have no rendered or
runtime consumer. The Mac app should compile from the same nouns it exposes.

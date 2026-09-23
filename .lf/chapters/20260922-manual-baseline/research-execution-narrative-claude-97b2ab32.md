# Research: how Loopflow's execution architecture evolved, summer 2026 → 2026-09-22

## Scope and evidence boundary

This is an independent reconstruction from primary sources read on 2026-09-22:
dated Wave memory (`wave/{product,infrastructure,intelligence}/MEMORY.md`),
`lf activity` merge receipts and branch names, `lf status` Task contracts,
current architecture docs (`docs/architecture/{execution,planning,homes,data,delivery}.md`),
and the review-chapter hardening notes in `scratch/review-chapter-hardening.md`.
No git or GitHub history was consulted, and the prior decentralization-framed
scratch artifacts were not used.

There is no formal chapter-start snapshot. "Beginning of summer" is anchored on
the earliest dated evidence in Wave memory (2026-06-30 through 2026-07-10);
"now" is the checked-out repository plus live `lf` read surfaces on 2026-09-22.
Planning ontology appears only where durable Work hands an objective to an
executor. Observation and interpretation are separated throughout; a final
section evaluates the "decentralization" framing, as directed, only after the
narrative stands on its own.

## The opening state: an execution model held together by its container

What the early-summer runtime looked like, from dated evidence:

- **The Mac client read runtime truth from a service.** Product memory's
  "Patterns (verified 2026-06-30)" section describes Concerto reaching remote
  `lfd` over HTTPS via Tailscale, with bearer tokens and cert trust as the
  remote data path. The later deletion note ("The HTTP-to-lfd-as-API path is
  **deleted**: `LocalWaveService` (~1500 lines) and `WaveServiceProtocol` are
  gone; ~22 consumers rerouted onto RegistryQuery") shows how much lifecycle
  logic that path carried at the start.
- **Names carried identity that code tried to parse back out.** The worktree
  redesign entry (PR #818, fixing the #802 fallout) records runaway nesting
  (`loopflow.jack-heart.bugs.20260705_1627.goals`, dated 2026-07-05 in the dir
  name itself), "wave identity that stopped resolving," and land rotation
  renaming a worktree out from under a running agent. The fix decoupled
  directory and branch projections and moved ancestry onto the `Run` record —
  the explicit lesson recorded: "the chain in a name is a *hint*, never parsed
  for truth."
- **Environment could decide what a process was.** Infrastructure memory's
  gotcha: "An earlier runtime chose between booting a listener and being a
  resident from inherited environment, so a promoted wave could attach to its
  parent's listener with the parent's token." Product memory elevates the same
  rule to an invariant: "Environment configures a process; it never decides
  what the process is."
- **Telemetry was a second, fragile account of execution.** Intelligence memory
  records the pre-057 ledger: 134 `run_id`s carrying more than one command,
  `lf runs` splicing two processes into one row, cost silently undercounted in
  28 multi-skill runs, and a silent best-effort write (`ledger_insert` at
  `debug!`) that let every ledger write on the machine vanish for 29.2 hours
  (2026-07-08 → 2026-07-09) while readers failed loudly. `lf usage` fetched
  `GET /v0/usage` from a running `lfd` — the sole consumer of the whole
  `lfd::client` module.
- **Basic lifecycle verbs were missing or ambiguous.** The 2026-07-10
  wave-controls dogfood found no single-wave stop; the fix made the listener
  "the sole cleanup owner" and taught surfaces that "failed bodies are
  attempts, not failed waves" — a projection change needed precisely because
  attempt failure and Work failure had been conflated.

**Interpretation.** The system knew a lot about the container around an agent —
tmux, environment, worktree names, service connections, journal rows — but no
one object cleanly answered the four distinct questions the failures kept
posing: what work persists, what is running now, what may be controlled, and
what happened historically. Several objects could each plausibly answer each
question, so recovery and presentation code kept picking the wrong authority.

## Phase 1 — July 18: delete Session, make Run the one executor

Observed via `lf activity`:

- **LOO-196**, branch `jack-heart/delete-session-and-make-run`, merged as
  PR #1099 on 2026-07-18. The branch name states the move directly: the
  Session-controller identity was deleted and Run became the execution path.
- **LOO-194**, branch `jack-heart/recover-run-routing` (and `-2`), merged as
  PRs #1098 and #1100 the same day: Run recovery across provider routing, so
  continuity stopped being bound to the first credential route or process.
- Product memory dates the settled vocabulary to 2026-07-19: "Work is stable
  identity, not a process. … A Run is one bounded period of execution
  authority; an AgentInvocation is one provider/process attempt inside it; a
  Turn is one observed provider boundary."

**Interpretation.** This was subtraction first: it ended a split brain in which
Wave, Project, and Task execution each had controller-shaped identities with
duplicate status and runner paths. But it also concentrated meaning: Run now
stood in for execution identity, liveness, history, and (implicitly) control.
The rest of the summer is the record of that concentration being taken apart.

## Phase 2 — July 20–23: the shared runtime meets real failures

Within days of the convergence, dogfooding produced a burst of merges
(the activity feed shows dozens of PRs across 2026-07-16 through 2026-07-23)
and a set of dated learnings that define the seams:

- **Retry without settlement** (dogfood 2026-07-21): LOO-167/193/195
  "repeatedly alternated between `ready` and a short-lived Run while `task` was
  absent from the installed flow catalog, producing hundreds of identical
  resumable failures." The recorded rule: missing lifecycle flows must become
  one durable blocked/failed boundary, not an infinite retry.
- **Containment liveness is not provider progress** (dogfood 2026-07-21):
  LOO-207 "reported `process_alive: true` while no exact `lf ps` receipt
  existed and `lf top` recorded no completed output." A live tmux/containment
  fact was being read as proof an agent was advancing. LOO-207's first PR
  (`make-managed-tasks-survive-missing`) started 2026-07-21; the task's serial
  chain ran into late August.
- **Controller evidence is not an agent Run** (learned 2026-07-20): when a
  merged PR completes a Task, persist the lifecycle transition directly —
  "never mint a synthetic Run to reuse a Run-owned terminal transition."
- **Persisted executable references are installed-state invariants** (learned
  2026-07-21): a stored flow name is not proof the installed binary can execute
  it.
- **One Task failure is one atomic durable fact** (learned 2026-07-21): failure
  event and Run/Invocation terminal state commit together; "an empty Run slot
  alone never authorizes retries."
- **One event-driven Home lifecycle for Wave startup** (decided 2026-07-21):
  attempt-scoped durable `live | failed` receipts, and "one failed Wave never
  terminates successful siblings."
- **Missingness became a first-class value** (learned 2026-07-21): "a provider
  receipt absent, one missing field, and a reported zero are distinct facts";
  an absent authority is a named `UNKNOWN`, never permission to infer.

**Interpretation.** Each failure has the same shape: one kind of execution fact
(a containment PID, a stored flow name, an empty Run slot, a parent's journal
state) was used as proof of a different kind of fact (provider progress,
executability, retry permission, child failure). The July learnings did not yet
redesign the model; they accumulated the rule that would drive August: evidence
from one domain may describe another domain but may not decide it.

## Phase 3 — August 20–28: history ≠ liveness, telemetry ≠ control, location ≠ identity

The dated correction wave, all observed in `lf activity` receipts and branch
names:

| Date | Task | Branch (intent as named) | PR |
| --- | --- | --- | --- |
| 08-20 | LOO-225 | `lifecycle-capability-contract` | #1207 |
| 08-21 | LOO-229 | `status-surfaces-must-distinguish-last` | #1225 |
| 08-21 | LOO-237 | `restore-task-launch-from-user` | #1214 |
| 08-21 | LOO-238 | `keep-concurrent-trace-capture-from` (killing work) | #1226 |
| 08-21 | LOO-207 | `make-managed-tasks-survive-missing` (2nd slice) | #1228 |
| 08-22 | LOO-248 | `make-pr-landing-a-watched` (process) | #1231 |
| 08-22 | LOO-253 | `main-agents-move-off-canonical` (main) | #1230 |
| 08-28 | LOO-265 | `make-task-promotion-recovery-depend` (-2) | #1237 |
| 08-28 | LOO-241 | `make-ledger-continuity-failures-match` (-2) | #1248 |

Grouped by the seam each one closed:

**Time: recorded state presented as present truth.** LOO-229's branch name —
status surfaces must distinguish *last* (recorded) from current — is the
correction for status replaying stale terminal facts as live ones. The current
`execution.md` states the end rule: an unterminated Run record "means only 'no
terminal proof was recorded.' It is not proof that a process is still alive."

**Observation: telemetry gating the work it observed.** LOO-238 kept concurrent
trace capture from terminating healthy Task work; LOO-265 made Task/promotion
recovery depend on something other than telemetry completeness (branch name;
the surviving contract in `execution.md`/`data.md` is that live process truth
comes from the OS, and optional streams "warn once and let the provider
continue"). LOO-241 made ledger-continuity monitoring obligation-aware so a
permanently red monitor would stop training people to ignore it — the explicit
lesson from the 29.2-hour outage. Interpretation: monitoring was demoted from a
control plane to a side channel, in code and in doctrine.

**Location: process placement confused with durable identity.** LOO-253 moved
main agents off canonical main into worktrees — the same isolation Tasks
already had, where durable work and delivery state survive the agent process
and each concurrent writer owns a distinct tree. At machine scale the same
split became Home: `homes.md` — "Home identity is stable; network route is
replaceable," and "Placement selects where Work belongs, not whether it is
currently running."

**Control: capability confined to the actor that can prove it.** LOO-225's
`lifecycle-capability-contract` and the infrastructure learning of 2026-08-22 —
"A live parent Run is not proof of child-control authority… only the immediate
parent may act" — converge on the current boundary contracts in `execution.md`:
"No `owner.json` means no durable cross-process signal authority. The direct
spawner may cancel the child handle it owns; another process may not infer that
capability from PID, Work, tmux, or Run-record evidence." LOO-248 gave PR
landing a watched process fenced by "exact recorded PR head plus landing
generation" (`delivery.md`) — a narrow lock where a real race exists, instead
of broad writer ownership.

**Overlay, not identity.** The 2026-08-27 revision made Task resident state "an
optional execution overlay" in `task_residents`: "there is no phase generation
or stale writer fence to turn one process into 'the Task.'" Generic Task-bound
Runs stay legal and gain no authority from attribution.

## Phase 4 — August 30: Session returns with a smaller meaning

Deleting Session as an executor (July 18) did not erase the real thing
providers call a session. PR #1250 (product memory, settled 2026-08-30)
reintroduced the word under a narrow contract:

- `lf session list --json` is the sole projection of unresolved human work.
- "Provider history is the resume authority" — resuming goes through Codex,
  Claude, or OpenCode's native command; "Closing a pane or provider exit
  resolves nothing."
- "tmux is only the first client's detached PTY cradle… not Session identity,
  readiness, presentation, liveness authority, or a resolution mechanism."
- The Mac multiplexer "presents Sessions without owning them."

The same period completed the daemon's demotion on the read side: all Mac data
reads converged on `RegistryQuery` shelling `lf … --json`; the HTTP-to-lfd data
path was deleted; "when `lf` and lfd disagree, `lf` wins"; and the governing
performance invariant became "a listener or Home process must never gate a
read." `lfd` remains a Home service keeper — listener reconciliation, webhooks,
landing claims (`homes.md`) — not the source of runtime truth.

**Interpretation.** This is the maturity check on the redesign: the
architecture stopped deleting a real external concept because the old internal
aggregate bearing its name was wrong. Session survived — stripped of Work
identity, process authority, and lifecycle ownership.

## The end state on 2026-09-22

What the current docs assert, read as one system:

- **Execution has no durable planning prerequisite.** `execution.md` opens with
  it: one Skill → one provider result → one Home-local Run record; attribution
  "describes the launch; it does not reserve Work or authorize a mutation," and
  a launch proceeds even when the planning store is unreadable.
- **Publish before spawn; settle once.** `CaptureHandle` atomically publishes
  an immutable manifest before the provider starts; optional `context.json` /
  `events.jsonl` are append-only evidence; `terminal.json` is exclusive-create,
  first writer wins; "Telemetry loss cannot hold settlement open or turn a
  successful provider result into failure."
- **Reads are disposable projections over Home-local files.** `scan_runs_since`
  rebuilds `RunSnapshot`; "There is no authoritative Run index to repair."
  Remote reads run the same reader on the target Home over `lf ssh` — "no
  implicit fan-out and no central Run database."
- **Controllers are boundary executors, not identities.** `planning.md`: a
  controller loads durable Work facts, executes one Flow boundary, records one
  monotonic transition, and can disappear; "A crash loses in-memory judgment,"
  nothing durable. `data.md` confirms the schema carries "no phase epoch,
  active controller slot, Task writer token, or Work ownership lease."
- **Each fact has one named authority.** `data.md`'s truth map assigns
  repository files, planning SQLite, journal JSONL, Run bundles, provider
  homes, Linear/GitHub, install receipts, and kernel locks each to a different
  owner, with the rule: "An identifier can join evidence across these sources.
  It does not transfer authority between them."

The honest cost is written into the docs as well: more `unknown`. An
unterminated record proves nothing live; a visible PID conveys no signal
authority; a remote Home may be unavailable rather than stopped. The system
declines to control some things it can see.

## What the boundary evidence says is still unfinished

Operational facts observable on 2026-09-22:

- **The observation layer lags the runtime it observes.** The review-chapter
  hardening notes record a Project launch that "produced an unterminated
  zero-stream Run before a replacement completed. Elapsed time and process
  lists could not prove whether the first attempt was live" — the exact
  `unknown` the architecture accepts, with no join yet to make it legible.
  They also record that `lf runs --json` capped its global list with no
  direct-child filter, and that recovering a child's conclusion required
  parsing provider-specific raw events (older Codex Runs discarded the
  completed final message). The current branch's uncommitted work adds
  `lf runs --parent` and `lf runs <run> --final` (documented in `execution.md`;
  the branch diff touches `run_record.rs`, `lf/commands/runs.rs`,
  `harness/codex.rs`) — readers, not new authorities.
- **Controllers still hit a centralized dependency execution itself shed.**
  `lf status product` shows both Project runners failing on 2026-08-28:
  "Project plan refresh blocked before the next phase: could not refresh
  wave/product from Linear: Stored linear token expired and automatic refresh
  failed." LOO-279 (created 2026-09-22) exists to fix unattended Linear OAuth
  refresh, citing "the three 2026-09-22 Project failures." Direct Skill
  execution tolerates a missing planning store; the end-to-end controller
  boundary that refreshes its plan does not.
- **Shared central state that remains is a live blast radius.** Product memory
  documents migration-number collisions on the shared `lfd.db` (product and
  intelligence both minting `061`) and an in-place edit of an applied migration
  taking down "*every* command sharing `lfd.db`." Interpretation: where one
  store still serves several owners, the failure mode the redesign removed
  elsewhere still occurs.
- **Fresh-database tests cannot see long-lived-store drift** (intelligence
  memory): CI stayed green while the only machine holding real history was
  broken. The trace/monitoring rebuild inherits this constraint.

**Interpretation — the next step implied by the evidence.** The redesign
separated authorities faster than it built the joins between them. What is
missing is not another executor or a restored central database, but a trace
that follows one execution across the boundaries — durable Work boundary, Home
placement, Run manifest, provider attempt and native session, exact process
evidence, terminal receipt, resulting Work transition, delivery outcome —
carrying each edge's source, freshness, and missingness, while remaining a
rebuildable projection rather than an authority.

## Observations vs interpretations

**Observed** (source-backed): the July 18 merges and their branch names; the
dated Wave-memory learnings quoted above; the August 20–28 merge receipts and
branch names; PR #1250's Session contract; the current architecture docs'
stated contracts; the 2026-08-28 Project-runner failures and LOO-279's
2026-09-22 creation in `lf status`/`lf activity`; the ledger outage and
migration-collision records; the review-chapter hardening observations.

**Interpretation** (mine): that the opening model's core defect was several
objects answering four different questions interchangeably; that July 18
temporarily over-concentrated meaning in Run; that the August work is best read
as four seams (time, observation, location, control) closing under one rule —
evidence may describe, not decide, across domains; that monitoring is now the
highest-leverage unfinished piece. No summer-wide deployment snapshot exists to
prove every machine occupied the reconstructed opening state simultaneously.

**Thin spots**: LOO-237's and LOO-265's precise mechanisms are inferred from
branch names plus the surviving doc contracts, not from read PR bodies; LOO-225's
content is known only as its branch name plus the capability contracts that now
appear in `execution.md`/`homes.md`.

## Assessment: is "decentralization" the right description?

Secondary — accurate as a description of the end state, wrong as the story of
how and why it happened.

It fits the destination: Run records are Home-local files with no central
index; remote reads execute on the target Home; providers keep their own
sessions; signal capability stays with the direct spawner; the daemon lost its
data-path role. But the summer's first decisive move was a *centralization* —
collapsing duplicate Session controllers into one Run execution path — and
several later moves centralized too (one reader path via `RegistryQuery`, one
vocabulary, one landing watcher per PR). Other moves pushed authority outward.
The through-line that explains both directions is the separation of identity,
continuity, control, and observation after repeated failures in which one
impersonated another. Authority ended up local because only the local actor —
the spawner, the kernel, the provider, the Home — can prove its kind of fact;
distribution is the consequence of that rule, not the goal. Framing the chapter
as decentralization also obscures the live counterevidence: the remaining
shared store is where the old failure class still bites, and the missing piece
(a cross-authority trace) is a deliberately *aggregating* — though
non-authoritative — artifact.

## Evidence sources

- `wave/product/MEMORY.md` — 2026-06-30 remote-lfd patterns; 2026-07-19 Work/Run
  vocabulary; 2026-07-21 dogfood (settle-don't-retry, containment vs progress);
  2026-08-30 Sessions revision; RegistryQuery convergence; migration collision.
- `wave/infrastructure/MEMORY.md` — worktree/WaveId redesign (PR #818); dated
  learnings 2026-07-20 through 2026-08-27; Wave startup lifecycle; landing and
  release evidence rules.
- `wave/intelligence/MEMORY.md` — ledger contract, 29.2-hour outage, fresh-db
  blindness, trace/span glossary.
- `docs/architecture/execution.md`, `planning.md`, `homes.md`, `data.md`,
  `delivery.md` — current contracts.
- `lf activity --since 120d --wave <w> --json` and `--task <id>` — merge
  receipts, branch names, LOO-278/LOO-279 steers dated 2026-09-22.
- `lf status product` — retained Task contracts and the 2026-08-28 Project
  runner failures.
- `scratch/review-chapter-hardening.md` — first-run observation-layer failures
  and the `--parent`/`--final` reader design.

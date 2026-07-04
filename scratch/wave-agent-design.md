---
requires: wave-reactive-server.md, wavechat-review.md, research/
produces: the wave agent design — product frame, forks, vision, MVP, reshape plan
---
# The Wave Agent

**The governing principle — waves outward (Jack, 2026-07-04).** This is a
radically decentralized, pubsub-everywhere vision: zero centralized control,
as a matter of spirit and integrity, and because the world it serves is one
where people are committed to their own workflows. The wave is the unit of
sovereignty; nothing sits above the waves. Coordination is shared fact (the
registry) and notification (pubsub), never command: the human steers by
messaging through the same doors as any process (`lf chat`), children
escalate upward by speech, Concerto observes without participating, and
`lfd serve` may only notify and gate — the moment it reimplements behavior
it has become a headquarters. Consistency is observational (facts + probes);
races are fixed with better facts, never with a coordinator. Every new
feature answers one question first: does this create a center?

**The mechanics of the principle: the wave process is the LISTENER; `lf`
runs are PUBLISHERS/CREATORS (Jack, 2026-07-04).** The server unifies the
wave by listening — the mind's stream, workers' rows and `lf chat` posts,
human messages — folding them into one timeline (journal → thread) and one
context field that flows back into every new publisher at birth (ambient
context). Publishing to no subscriber drops the message (`lf chat` outside a
wave exits 0 — correct pubsub semantics, not degraded mode). The mind is
just a publisher whose stream the listener supervises directly. Listener
downtime never stops publishers; the restarted listener reconciles from the
substrate (snapshot-then-delta, already built). Concerto is a second
listener; **lfd-serve is a relay listener AND an access gate (Jack)** — 
outbound it re-publishes the machine's substrate to remote subscribers
(push, the one thing transient lf invocations can't be); inbound it is the
authenticated doorway for remote speech and queries, the only place
identity/teams/permissions live (on-machine there is no gate — adding one
would be adding a center). Both are boundary functions: the gate controls
the doorway into the machine, never the waves behind it. Durability gap,
small: a publication addressed to a down listener errors today — future:
store-queued publications drained at boot.

Loopflow's evolution: from a way of launching agents into a toolset used by
higher-level agents to execute progress through lower-level agents. The Wave —
a continuous stream of ongoing work, attached to an Asana roadmap — gets its
own always-on agent. This doc unifies the product frame, the architecture
forks, the long-term vision, and the MVP for this branch, grounded in five
deep reviews of the tree (wave runtime, conversations harness, Concerto
WaveChat, old-infra map, prior-art distillation).

## 1. Product frame

**The wave agent is a colleague, not a dashboard.** You DM it like a teammate;
Asana is its task list; PRs are its output; Concerto is its presence. Chat is
the primary interface, not a debug console over a loop. This is the single
framing decision everything else serves — and it reverses the earlier "chat is
talk-only" scaffold decision (flagged in §5, Jack's call).

**State decomposition.** The roadmap→Asana move generalizes: state leaves the
repo when humans co-own it in a tool they already live in; the repo keeps what
belongs with the code.

| State | Writer | System of record | Projection |
|---|---|---|---|
| Intent (GOAL.md) | human | repo | — |
| Roadmap | human + agent | Asana | (mirror deleted) |
| Memory / learnings | agent | **wave event log** | MEMORY.md checkpoint |
| Activity / conversation | agents | **wave event log** | thread (Concerto) |
| Decisions / attention | both | lfd store (AttentionItem) | Concerto queue |
| Operational state (runs, sessions, PRs) | lfd | lfd store | Concerto |

**Memory is two artifacts with different jobs, not one substrate.** The
**event log is what happened** — runtime truth for the live agent (replay,
resume, thread, state); machine-shaped, per-machine, never committed.
**MEMORY.md is what we know** — deliberately curated by the mind (not
mechanically rendered from the log), versioned in the repo, readable by any
toolchain. It earns its place twice over: subagents share context through it
with zero machinery, and `GOAL.md + MEMORY.md` is the *serialized agent* —
anyone can launch their own version of this agent in a completely different
tool chain. The agent's identity is markdown in the repo, not proprietary
state (this is also what reaches the A2 cloud path, which clones fresh with
no server). Write discipline: the mind is the sole curator; subagents read,
never write — which dissolves the concurrent-write races without any
locking. The Dumb Zone finding (silent degradation past ~40% context fill;
MEMORY-as-growing-blob is the anti-pattern in slow motion) makes curation a
hard discipline: distilled facts and constraints, not accreted turn dumps.

## 2. The forks, and the composition

Four load-bearing choices. The recommended composition is **B — "the
colleague."**

### Fork 1 — what runs the wave agent's mind
- **1a** one-shot vendor passes (`codex exec` per pass) — what's built; coarse.
- **1b** long-lived vendor session (codex app-server / claude resume-chain /
  opencode serve) — what the salvaged harness *is*; real mid-turn
  `turn/steer` + `turn/interrupt` on codex; vendor tools and subscription
  pricing come free.
- **1c** true in-house harness (direct model API, own context assembly) —
  full 12-factor control; but API-token pricing for a 24/7 agent and a
  rebuilt tool ecosystem.

**Decided: 1b (Jack, 2026-07-04).** 1a is out on two grounds: it forfeits
Max-subscription leverage for Claude, and one-shot spawns never yield a
portable, reattachable session for interactive launches — a long-lived vendor
thread (durable thread id, resume-capable) does. 1c is out on the same
subscription grounds. The harness review found the codex driver already
speaks app-server JSON-RPC with steer/interrupt — option B of the old "big
fork" is sitting in the tree while the wave server ships option A. Keep the
`Harness` trait as the boundary so a 1c backend can slot in if vendor
coupling ever bites.

### Fork 2 — who manages the workers
- **2a** the harness holds per-worker handles (HumanLayer daemon shape).
- **2b** the agent manages workers through tools (`lfq worker run`,
  `lfq sessions`); the server supervises processes and observes.

**Decided: 2b with full instrumentation (Jack, 2026-07-04).** Orchestration
intelligence lives in the prompt; loopflow is the toolset. But `lf` must
instrument every launch so the system stays fully monitorable: whenever an
agent invokes a flow (`lf design`, …), the invocation auto-registers — wave
attribution known, session findable, openable in an embedded terminal in
Concerto. The seam exists: `LFD_WAVE_ID`/`LFD_SESSION_ID` env already
propagates inside managed sessions and `lfq` infers wave/parent from it. The
gap is direct `lf` invocations: today only `lfq worker run` creates a Session
row; a bare `lf design` inside a wave context is invisible. Fix: `lf`
self-registers as a child session (tmux-wrapped) whenever it starts inside a
wave context. The rest of the substrate exists end-to-end: capacity checks,
Run rows, `parent_session_id`, Concerto attach, PR auto-create, repair
chains. 2a would have built a second worker-management layer next to a
working one.

**Worktree naming carries ownership.** Extend the sibling convention so the
filesystem itself attributes work: `<repo>.<wave>` = the wave's own worktree
(the mind's, today's shared per-wave tree); `<repo>.<wave>.<id>` = one per
dispatched worker (`<id>` = short run id, tying the directory to its Run row);
`<repo>.<name>` = human feature worktrees, unchanged. Segment count signals
ownership (two = human, three = wave worker); reaping a wound-down wave is a
prefix prune that can't touch human trees. Implementation notes: worktree
recognition + `lf op wt prune` + land rotation must learn the three-segment
form, and tmux name derivation keeps its `.`→`-` sanitization.

**Placement is a per-dispatch decision (Jack, 2026-07-04).** Wrong-cwd
launches aren't the worry; first-class worktree management from `lf wave`
down is. Replace the executor's `workers == 1` shared-vs-per-run heuristic
with an explicit placement on every dispatch:

- `fresh` (default) — new `<repo>.<wave>.<id>` worktree off main;
  independent PR, independent land.
- `pool` — run in an existing worktree (the mind's, or a sibling's); a
  *conscious* opt-in to tightly-coupled parallel edits; the pooler owns the
  collision risk.
- `stack <run>` — new worktree whose branch forks from the parent run's
  branch, for dependent series. Stacking is branch lineage, not directory
  lineage — the filesystem stays flat, git carries the DAG.

Mostly promotion, not construction: runs already carry `parent_run_id`,
`stack_position/group/status`, `target_branch`, and the queue
reconciler/`advance_branch` handle landing; per-run worktree creation exists
behind the workers>1 path. Surface as `lfq worker run --pool | --stack
<run-id>`; record placement on the `WorkerDispatched` journal event; give the
mind placement guidance in its operating prompt (default fresh; pool only
when workers must see each other's edits live; stack when the task names a
dependency). MVP tension, held: **stacks are serial by default** — the mind
dispatches level N+1 only after N lands; restack-cascade automation for
parallel stacks is deliberately deferred.

### Fork 3 — one mind or two
- **3a** one mind, two input streams: the same agent context handles progress
  and chat; chat *is* steering.
- **3b** two minds: a grinding progress agent + a cheap chat responder over
  projections.

**Decided: 3a, refined by 2b (Jack, 2026-07-04).** With many subagents, a
pure 3b spokesperson world doesn't hold together; the shape that does is "a
long-running chat agent that also does top-level progress" — which is 3a once
the mind never grinds. Under 2b the mind's personal work is thin
orchestration (read worker summaries, check roadmap, dispatch, fold memory,
answer); turns are tens of seconds, so chat is responsive without a second
mind, and app-server steer covers mid-turn. This is loopflow's own
coordinating-session discipline promoted into architecture: decisions,
sequencing, reading results back — never inline grinding. Two consequences:
"constantly looping" becomes event-driven + heartbeat (workers grind in
background; the mind wakes on worker-completed / message / roadmap-change
events, plus a heartbeat tick to proactively start the next roadmap item when
quiet); and the context-fork discipline (trust summaries, never re-read
worker transcripts) becomes the mind's survival mechanism against the Dumb
Zone, not a guideline. Known failure mode: the mind is only as good as its
workers' summaries — worker report quality is load-bearing.

### Fork 4 — state substrate
**Decided, with a correction (Jack, 2026-07-04): log-as-truth for the live
agent; MEMORY.md is one of its inputs, not a projection of it.** The
log-vs-file framing was a false dichotomy. The append-only event log is
runtime truth — thread, state, status/cost are folds/rebuildable indexes over
it, and it's what makes resume/fork/replay cheap. MEMORY.md is a separately
curated artifact with its own jobs (subagent context sharing; toolchain-
portable serialization of the agent — see §1). No event log exists anywhere
in lfd today (EventHub is ephemeral broadcast) — this is net-new, and it's
the one decision expensive to retrofit. Store = truth, bus = best-effort
liveness (notify + refetch on reconnect).

### Composition B, in one paragraph
`lf wave <name>` runs a per-wave server. Its mind is one long-lived vendor
session (codex app-server first) driven through the salvaged harness. Every
harness event appends to a per-wave event log; the thread, memory seed, and
wave state are folds over it. User messages append to the log and reach the
mind as steer ops or queued turns — append-and-coalesce, never
reject-when-busy. The mind executes progress by dispatching workers through
`lfq worker run` (existing tmux/Run/PR plumbing); Concerto renders the thread
over SSE and attaches to workers over tmux. Interrupt is three-scoped: abort
the mind's turn / interrupt one worker (a tool call) / tear down the wave.

## 3. Long-term vision

The big version, in the order the primitives unlock it:

1. **Deterministic replay.** The wave is a pure fold over its log; control
   flow is testable without live model calls; any incident replays.
2. **Fork-to-explore.** Resume-as-fork from any point in the log: run two
   plans in parallel from the same state, keep the winner. The lineage
   machinery (parent ids, immutable ancestors) exists in prior art only for
   crash recovery; we use it for speculative branching.
3. **Part-grained wire.** Message/Part with lifecycle as the streamed
   primitive (token deltas, in-place item mutation) replacing the turn-grained
   wire — the internal log already stores fine-grained events, so this is a
   wire upgrade, not a model rewrite.
4. **Decisions as first-class HITL.** Durable, addressable, idempotent
   records with deny-with-comment feeding correction back; inline in the
   transcript at the turn they gate; gating as a knob per wave (autonomous ↔
   supervised glyphs). Built on AttentionItem + a reply channel.
5. **Multi-wave attention control plane.** The wave list becomes the human's
   outer loop: dense keyboard-first table where status = attention; archive
   completed; OS notifications only on gates. The human lives in the list and
   descends into WaveChat to steer.
6. **Cost as a control signal.** Per-turn usage accrues onto the log; waves
   pace, downshift models, or escalate against a `spend_cap` (the
   2-wave-budget design rides this spine).
7. **Wave trees.** Parent/child waves (ancestry landed in #781); children
   draw on parent headroom; algedonic escalation routes child→parent→human.
8. **Rented persistence.** The same goal + committed MEMORY checkpoint +
   `.mcp.json` run as cloud routines (2-looping-agent-cloud A2); the wave
   server is the local, richer tier of the same product.
9. **lfd's second life.** After the collapse (lf does the work, the db is
   the registry), the server earns its way back only as push: the guarded
   subscription server for Concerto today; later a global pubsub fabric
   (GitHub events, cross-repo signals) and the access/query gate for
   distributed compute — multiple machines, teams. Waves stay sovereign;
   lfd federates them.

## 4. The MVP (this branch)

Large, end-to-end, works for one setup (loopflow building loopflow), drives
real user behavior: **you open Concerto, watch a wave grind, talk to it, steer
it, and its workers land PRs.**

### In scope
1. **Event log spine.** Per-wave append-only JSONL under the wave's data dir
   (`.lf/` or `wave/<name>/`; not committed), written by one supervisor task;
   replay on boot ends restart amnesia (and fixes the two-generations
   transcript corruption). Thread and memory-seed are folds. Wire stays
   turn-shaped for now (§3.3 is vision) — but the log records events finely.
2. **The mind on the harness (1b).** Replace `SubagentSpec::codex_progress` +
   `engine::stream` with the codex app-server driver. One persistent thread;
   orchestration turns are triggered by events (worker completed, message
   arrived) plus a heartbeat tick when quiet — not a busy-loop — each seeded
   from folded context (`<goal>`, `<wave_memory>`, `<roadmap>`,
   `<in_flight>`, `<last_failure>`). The mind never grinds inline; its system
   prompt carries the coordinating-session discipline. Capture and persist
   the vendor thread id as the first durable act (resume beats cold-respawn).
3. **Chat = steering (3a).** `POST /messages {op: message|steer|interrupt}` —
   explicit op, no inference (the OpenCode #16102 lesson). Message log is the
   queue; append-and-coalesce; steer drains as `[STEER]` guidance at the next
   boundary; interrupt = cooperative cancel → grace → finalize partial as a
   well-formed **interrupted** record (a value, not a crash).
4. **WaveState machine.** `idle | running | waiting_input | interrupting |
   interrupted | failed` with a `can_transition` guard table and a startup
   janitor (the HumanLayer stuck-`interrupting` lesson). On `/health`, in the
   log, and as SSE events — the composer keys its verb off it.
5. **Workers through lfd (2b), fully instrumented.** The mind dispatches via
   `lfq worker run`; in-flight state folds into each pass's seed (existing
   `list_in_flight_dispatches`). Worker transcripts are *not* ingested — the
   parent trusts summaries (context-fork discipline); Concerto attach covers
   descent into a worker. Instrumentation: any `lf` flow invocation inside a
   wave context self-registers as a child session (tmux-wrapped, wave
   attributed) so no subagent is ever invisible to Concerto.
6. **Live wire.** Serve the open turn (`TurnBuilder::snapshot` finally gets
   its consumer); SSE dedupe becomes id-replace so in-progress turns update
   in place — the Swift client already handles this. Un-disable the composer;
   phase-dependent verb (Send / Queue / Interrupt); failed sends restore text.
   *Named divergence (built 2026-07-04):* the SSE carries full whole-turn
   snapshots per delta, not notify+refetch (the HumanLayer-conservative call
   wavechat-review recommended). Snapshot + id-replace semantics make a lossy
   bus self-healing — a dropped frame is corrected by the next one — except
   the terminal frame of a turn, which has no successor; a lagged client can
   show `running` until reconnect. Accepted for latency + client simplicity;
   backstop (idle keepalive re-send or client refetch-on-quiet) is future
   work.
7. **One brain per wave, enforced.** `lf wave` is THE wave brain. Kill the
   loop_ticker path for served waves (pause-on-serve or mode gate) — the
   split-brain is a live bug today, not a hypothesis.
8. **Safety floor.** `kill_on_drop` + process-group kill on the mind's child
   process; progress runs in the wave's worktree, never the main checkout;
   MEMORY auto-append-blob removed (the mind updates MEMORY.md deliberately;
   the log carries the raw history).

### Out of scope (deliberately)
Decisions/HITL records and gating UI; part-grained deltas; multi-wave
attention table; cost accrual; wave trees; worker-stream ingestion; opencode/
claude drivers for the mind (codex first — the drivers stay, conformance-
tested, as the vendor-spread seam); Python `chat_turn` mirror until the wire
shape settles (carve-out noted in the fixture README).

### Runtime data structures (settled 2026-07-04)

```rust
struct Event { seq: u64, at: Timestamp, kind: EventKind }   // one JSONL row

enum EventKind {
    // conversation
    UserMessage    { id: MessageId, op: MessageOp, text: String },
    TurnStarted    { turn_id: TurnId, answers: Vec<MessageId> },  // consumption marker
    TurnItem       { turn_id: TurnId, item: ConversationItem },
    TurnFinished   { turn_id: TurnId, status: TurnStatus, usage: Usage },
    // mind lifecycle
    ThreadStarted  { vendor: String, thread_id: String },  // borrowed handle; FIRST durable act
    MindState      { from: MindState, to: MindState, reason: String },
    // orchestration — observations, not commands (server tails lfd's event
    // stream filtered by wave_id; the mind's lfq call is intent, recorded as
    // a TurnItem; these are the confirmed facts)
    WorkerDispatched { run_id: RunId, session_id: SessionId, flow: String, task: String },
    WorkerFinished   { run_id: RunId, outcome: WorkerOutcome, summary: String },
    // memory
    MemoryUpdated  { summary: String },   // mind curated MEMORY.md; diff lives in git
}
```

**Every projection is a fold.** Thread = conversation events. State = last
`MindState`. **Queue = `UserMessage`s not yet named in any
`TurnStarted.answers`** — the turn declares what it consumed, so "queued" is
pure fold, no separate inbox to desync (the OpenCode lesson), and the UI can
honestly render "queued, addressed next turn". (Decided.)

```rust
enum MindState {
    Idle,                                  // thread alive, no turn in flight
    Turning      { turn_id: TurnId },      // one turn generating / tool-calling
    Interrupting { turn_id: TurnId },      // cancel fired; cooperative → grace → kill
    Failed       { reason: String },       // thread dead, retries exhausted; algedonic
}
// WaitingInput joins when Decisions land — not before.
```

The machine is about the **mind only** (two-axes split: workers grinding is
not a mind state; wave-level display = derived `(mind_state,
workers_in_flight)`). A failed *turn* is `TurnFinished{status: Failed}` and
the mind returns to Idle; `Failed` is reserved for the mind itself.
Transitions go through a `can_transition` table (illegal = bug: logged,
refused), every transition appends a `MindState` event, and the janitor
bounds the transients (Interrupting past deadline → force kill → Idle;
Turning with dead child → finalize failed → Idle). One screen, on purpose.

```rust
enum MessageOp {
    Message,     // append; queued; next turn answers it
    Steer,       // inject into the current turn (app-server pending_input);
                 //   falls back to Message when idle
    Interrupt,   // cancel current turn, finalize as `interrupted`;
                 //   non-empty text → becomes the next turn ("interrupt & send")
}
```

Explicit op at the API — no inference (OpenCode's unresolved ambiguity).
Composer mapping: idle+text=Message; turning+text=Steer (Interrupt&Send one
modifier away); turning+empty=Interrupt.

Wire model stays `ChatTurn` + `ConversationItem` for the MVP, with one
lifecycle enum (`pending | running | completed | failed | interrupted`)
replacing the five-enum lattice.

### Done-when
- `lf wave goals` survives a restart with its full thread intact (log replay).
- Send a message mid-pass → it lands in the thread instantly, the mind
  addresses it at the next boundary; steer op visibly redirects the pass.
- Interrupt mid-turn → partial turn finalized as `interrupted`, wave goes
  `idle`, no orphan codex process (verified by `ps`).
- The mind dispatches a worker via lfq; the worker's tmux session appears in
  Concerto; its PR lands; the next pass's seed shows it.
- Concerto renders in-progress turns streaming, composer never dead.
- Old loop cannot double-drive a served wave.

## 5. Reshape plan (deletions and fixes to match the design)

From the five reviews — each item is a review finding, not speculation.

**Delete now (dead under every composition):**
- `types.rs` dead records: `Conversation`, `ConversationConfig`,
  `ConversationStatus`, `TurnStatus`, `TurnUsage` (as wire), `ContextSnapshot`,
  `DocumentEntry`, `ItemDelta`, `PersistedConversationEvent`,
  `CreateConversationParams` — reference a persistence layer removed in
  `45ab5d36e`; zero callers.
- `Supervisor::labels()`; the doc-drift comments (subagent.rs:58 "worktree",
  progress.rs:29 "supervised task").
- Swift orphans: `TerminalWorkspaceView` (no mount point),
  `TerminalAttachCommand`, `PortfolioRepoState.attachWaveAgent`, dead
  `Phase.failed`.
- `RunWaveRequest.roadmap_item` file-path validation (contradicts
  Asana-is-the-roadmap).
- `lf goal`'s looping duplication — `lf wave` is the brain; goal *rendering*
  internals stay (render_goal is load-bearing for seeds and cloud).

**Restructure (the MVP core):**
- `runtime.rs`: `Mutex<Vec<ChatTurn>>` → event log + folds (~5 call sites).
- `subagent.rs`/`progress.rs`: one-shot exec → harness-driven persistent
  thread; supervisor grows cancel tokens + parent links or shrinks to
  `JoinSet` (currently ceremony).
- `compose_reply` template → gone; chat goes through the mind.
- Status enums: five → one item lifecycle
  (`pending|running|completed|failed|interrupted`) + turn status gaining
  `interrupted`; `finish_open` stops stamping interrupts as `Failed`.
- Harness trait: split `interrupt()` from `stop()`; blind auto-approve
  (`codex.rs:277` "accept", `opencode.rs:459` "always") becomes an explicit
  policy knob (auto-approve default for now, Decision surface later);
  `ProviderSessionId` out of the event channel.
- Codex conformance test: extract the reader's dispatch into
  `process_notification()` so the test pins production code, not a copy.

**Fix regardless (wire/DTO):**
- Strip `#[serde(default)]` from `ConversationItem`/`FileEdit` (Swift decodes
  those fields as required — live split-brain).
- `command: Vec<String>` semantics: pick argv, fix claude (whole-line) and
  opencode (whitespace-split) mappings.
- Server SSE: id-replace dedupe; serve open turn; client clears state on
  stream (re)open.

**Keep verbatim:** server.rs HTTP+SSE shell, `.wave-endpoint` discovery, the
three vendor mappings + captured traces (~2,900 lines, the expensive asset),
`ChatTurn.swift` DTO discipline, the WaveChat rendering stack, memory.rs
mechanics (as checkpoint writer), `opencode_runtime.rs` reaper (wire
`reap_orphaned_opencode_servers` into startup when opencode returns).

## 6. Decisions

**Decided (Jack, 2026-07-04):**
- **Fork 1 = 1b.** Long-lived vendor sessions; Max-subscription leverage;
  portable/reattachable sessions for interactive launches.
- **Fork 2 = 2b + instrumentation.** Agent orchestrates through tools; every
  `lf` subagent launch auto-registers with wave attribution, findable and
  embeddable in Concerto.

- **Fork 3 = 3a refined.** One mind that orchestrates, never grinds; chat is
  steering by definition (supersedes the earlier "talk-only" decision);
  event-driven + heartbeat, not a busy-loop.

- **Fork 4 = log-as-truth for the live agent; MEMORY.md a curated input,
  not a projection.** Cadence question dissolved: the mind curates memory as
  it learns; commits ride landings.

- **Journal location = per-wave JSONL under `.lf/`** (server-owned
  persistence, not IPC; gitignored, per-machine).
- **`lf goal` deleted this branch** (Jack, 2026-07-04). `lf wave` is the one
  brain; goal *rendering* internals (`render_goal`, `load_goal`) stay — they
  seed the mind's turns and the A2 cloud path.

**Corrected (2026-07-04, per the architecture roadmap item "Collapse lfd/lfq
into lf; shrink lfd to a guarded subscription server"):** there is no central
daemon above the wave. `lf` is one binary that does the work AND records to
the shared local SQLite store directly — **the db IS the run registry**
(`register_session` / `active_sessions_by_worktree`, grouped by worktree
basename). `lf wave` runs its own server, inside its own worktree
(`<repo>.<wave>`, self-bootstrapped). Decision point three's *facts* stand —
the mind registers as a WaveAgent session, one-brain keys on that fact,
worker activity is observed — but the transport is store-direct, not HTTP/WS
to a daemon. Worker dispatch is `lf q worker run` (worktree + tmux + store
rows, no HTTP). `lfd serve` survives only as a guarded subscription server
(push for Concerto), execing `lf` rather than reimplementing behavior; hard
cut, no compat shims. Its future beyond that: a global pubsub fabric (e.g.
GitHub events) and/or the access/query gate for distributed compute across
machines and teams — the thing you consult, not the thing that runs you.

**The calling convention (2026-07-04, Jack): one door — exec.** Mind ↔
loopflow is one interface, a small vocabulary rendered as `lf` commands:
`lf q worker run` (dispatch), **`lf chat`** (post to a wave's thread),
**`lf memory`** (read/update MEMORY.md via the server), later
`decide`/`status`. The binary IS the harness with an exec entry point; the
same commands serve minds, workers, humans, and scripts. The decisive
argument: exec is the only door available to *every process on the machine*
— a worker deep in tmux can `lf wave say` a rich completion report, which
tags (parseable only from supervised streams) never could. Agents heredoc
all day; prose-through-shell is not a hardship. Tags (`lf_tag.rs`) and MCP
remain possible future *renderings* of the same vocabulary (MCP = the
maximum-compliance door if vendor posture tightens), not MVP surface.

Semantics that keep it clean:
- **Reactive vs proactive speech**: replies to a user message are the turn's
  own text (answers return on the channel the question came in); `lf wave
  say` is for initiations — mid-work FYIs, worker reports.
- **Attribution via the registry**: `lf chat` stamps `LFD_SESSION_ID` from
  env, so the thread knows mind vs worker vs human; worker reports arrive
  pre-attributed to their run.
- **Wave-tree routing (Jack)**: `lf chat` defaults to the invoking context's
  own wave (env, else worktree name); `--parent` walks `parent_wave_id` in
  the store and posts to the parent's live server — its endpoint rides the
  parent's WaveAgent registry row, so cross-repo parents resolve through the
  store, not the filesystem. `--wave <name>` targets explicitly. This is the
  algedonic channel's transport: child minds escalate/report upward through
  the same verb workers use; root + `--parent` errors until Decisions give
  it the human fall-through.
- **Single-writer preserved**: `chat`/`memory` POST to the live server via
  its endpoint — new doors into the same choke point, never a second
  journal/file writer. The server holds MEMORY.md's pen (also fixing the
  live bug where the mind's file tools edit the worktree copy while seeds
  read the origin's).
- **Intent-then-fact journaling**: command-item = intent; the journaled
  emission/observed effect = fact. Same discipline as WorkerDispatched.
- **Hierarchical scopes (Jack, 2026-07-04; post-demo, rides the lf-language
  item)**: chat and memory are two-level — wave-global (the thread +
  MEMORY.md) and branch/worktree-scoped (a channel + memory overlay per work
  line), the latter strictly *extra on top* of the former (overlay
  semantics, like the `.lf/` override model). Publish to either — `lf chat`
  defaults to the publisher's own scope (speak locally, escalate
  deliberately); subscribe to either or all — **the wave's listener
  subscribes to all its children's scopes** (and a parent wave's listener to
  its child waves', same move one level up). Mechanically: a `scope` field
  on journal events; channels are folds-with-a-filter, no new storage.
  Branch memory lives in the worktree — travels with the branch; at land the
  mind curates what folds up into wave memory (ephemeral work gets ephemeral
  memory; only distilled learnings survive the merge).

**Concerto is a viewer, never a participant (2026-07-04, Jack).** Concerto ↔
lf runs entirely on shared machine substrate — store rows, `.wave-endpoint`,
tmux, per-wave SSE — nothing routes through Concerto and no loopflow harness
"manages" it. No dashboard-in-the-loop failure mode; extra viewers are free;
its writes are the human's acts through the same doors as any process.
On-CPU tracking ends at the CPU: remote viewing (Mac Mini/Tailscale, teams)
is the shrunken `lfd serve`'s whole job — a gate that execs lf and watches
the same store, keeping remote Concerto a viewer one hop removed. (Gap on
the books: fleet surfaces still call fat-daemon routes; they migrate with
the collapse.)

**Vendor-policy posture (2026-07-04).** The blessed pattern is *wrap the
vendor's product, don't replace it*: minds ride official surfaces (codex
app-server — their published integration point; claude via the official CLI,
the HumanLayer-precedent shape), subscription credentials never touch raw
APIs, volumes stay human-scaled (one mind per wave, slow heartbeat, every
session attachable in the vendor's own surface). Public positioning:
"orchestrates your Claude Code / Codex sessions." Keep hard-case competitor
names out of committed docs. MCP stays the maximum-compliance rendering of
the emission vocabulary if posture tightens; the 1c API-key seam remains the
always-compliant escape hatch.

**Named question — is the socket essential? (Jack, 2026-07-04.)** The wave
*process* is non-negotiable — it is the mind's KEEPER: keeps the vendor
thread alive, keeps it accountable (journaled turns, coalesced queue with
real answers, bounded interrupt, failure cap + revival), holds the two pens
(journal, MEMORY.md), and watches the store. The HTTP *surface* is
contingent: every route has a substrate-only equivalent (speech via a
watched store/spool; reads via journal folds — ambient context already does
them; push via file watching; one-brain via registry row + pid probe).
Kept for now: it buys one canonical fold (dropping it makes the fold a
cross-language contract — the split-brain class the DTO rules kill), ack'd
writes, and it's built and demoed. Governing rule: **the server is an ear,
not an organ** — nothing may depend on HTTP specifically; the lf-language
item designs the unified stream so a journal tail could replace it without
touching the vocabulary. Corollary (the wave spectrum): a wave exists as
data with nobody home — **dormant** (substrate only; lf chat drops,
correctly) → **observed** (viewers reading) → **minded** (`lf wave`
running). The process adds animation, not existence.

**The doorman, not the daemon (Jack, 2026-07-04 — "are we coming back to
lfd as global server?").** Old lfd was a center because it OWNED things
(executor, ticker, dispatch) — traffic had no way around it, not because it
had a port. What remote Concerto needs is an **aggregating doorman**: one
optional per-machine viewer-with-a-socket that reads store + tails journals,
re-publishes one HTTPS/SSE surface (fleet + threads), and forwards inbound
speech by exec'ing `lf chat`. Four constitutional tests keep it a doorman
forever: (1) **route-around** — locally everything it serves is readable
from substrate without it; (2) **writes only through the doors** — every
mutation execs `lf`, no privileged channel; (3) **crash-harmless** — derived
state only, stateless restart, no work changes course; (4) **non-exclusive**
— two can run; centers are things there can only be one of. If per-wave
sockets are ever dropped, the doorman becomes the machine's only socket —
exactly when these tests matter most.

**Open:** none blocking.

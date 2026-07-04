---
requires: wave-reactive-server.md, wavechat-review.md, research/
produces: the wave agent design — product frame, forks, vision, MVP, reshape plan
---
# The Wave Agent

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

**Open (Jack's remaining calls):**
1. **Log location**: per-wave JSONL file (recommended for MVP; server-owned,
   not IPC) vs an lfd store table (couples the wave server to the daemon).
2. **`lf goal` fate**: fold into `lf wave` now, or keep as the lfd-less
   Concerto launch primitive one more cycle?

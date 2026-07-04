---
requires: none
produces: the wave conversation/control data model
---
Review surface for the wave server's data structures + control flow, informed by
how Codex, OpenCode, and HumanLayer solve queue / interrupt / steer.

## The "turns" problem (why the current model is wrong)

Current #794 model = one `Vec<ChatTurn>`, each turn request→response and expected
to complete. Reality: autonomous background work with no triggering message,
concurrent runs, long-lived in-progress activity that never resolves to a tidy
turn. All three references reject "turn" as the primitive:

- **Codex** streams **items** (`item.started → updated → completed`, stable id);
  `turn.*` are just bookends. Consumer keeps a map keyed by item id, mutates in
  place.
- **OpenCode** streams **message parts** (text/reasoning/tool/agent) with
  token-level `part.delta`; the **message log is the source of truth**.
- **HumanLayer** models a **session state machine** + human **decisions** as
  first-class records — neither is a chat turn.

**Convergent answer:** the primitive is the **item/part with a lifecycle**
(stable id, `in_progress` first-class), streamed as deltas. The **thread is a
projection** over a log, not the storage model. "Turn" is just the thin wrapper
when items answer a message; background progress is the same items, no wrapper.

## Proposed model

```
Message          a log entry: { id, role: user|assistant|system, parts: [Part], created_at }
Part / Item      the primitive: { id, kind, status, ...payload }
  kind = text | reasoning | command | file | tool | message
  status = pending | running | completed | failed | interrupted     (lifecycle, streamed by delta)
Run              a supervised unit of work (subagent) in the tree:
                 { id, parent?, label, state, started_at, ended_at? }  — emits Parts, independent lifetime
Thread           projection: the ordered Messages a client renders (NOT stored separately)
Decision         first-class HITL record: { id, spec (tool+input | question+options), status (approved|denied, comment, by, at) }
WaveState        idle | running | waiting_input | interrupting | interrupted | failed
```

The **message log IS the queue** (OpenCode's strongest call): `POST /messages`
appends; the progress loop re-reads the log at each step boundary. No separate
in-memory inbox for the common case → it can never desync from `GET
/conversation`. A real queue only for UX affordances (pin / send-now / cancel),
kept client-side.

## Control flow

**Queue.** Append-and-coalesce, never reject-when-busy (OpenCode's
`assertNotBusy`/`BusyError` is the root of their queue-loss bugs). One supervised
progress run per wave; a message arriving mid-run attaches to it (Runner
`ensureRunning` coalescing), consumed at the next step boundary.

**Interrupt.** Cooperative cancel token (threaded into the LLM call *and* each
subagent, cascading by parent) → short grace (~100ms, Codex) → hard abort.
**Finalize the partial** as a well-formed closed record: stamp `interrupted` +
completion time on the open item, mark orphaned tool calls interrupted, go idle.
Cancellation resolves to a **value, not an exception** (OpenCode). **Task-scoped**
— abort the agent turn, never tear down the wave/background (Codex keeps
background terminals alive; make "abort turn" and "tear down wave" distinct ops).

**Steer.** Step-boundary only — none of the three do true mid-token injection.
Two flavors; **pick one, don't be ambiguous** (OpenCode's open pain point):
- *new turn* — the message is re-read as the next user turn (implicit).
- *tagged injection* — drained at the top of the loop, spliced into the next
  model call as `[STEER]…[/STEER]` guidance for the *current* objective.
Recommend **tagged injection** for a goal-directed progress loop.
HumanLayer's floor is coarser (**interrupt + continue** with a new query); worth
having as the explicit "redirect the wave" op even if steer handles nudges.

**Decisions (HITL).** Model a pending human decision as a durable, addressable
record (spec + status), resolved by an **idempotent** `POST /decisions/{id}`
(409 on re-decide), with `deny-with-comment` feeding correction back without
killing the run. Surface via a `waiting_input` wave state + `decision.pending` /
`decision.resolved` events — the client renders "blocked on X" from *state*, not
by scraping the thread.

## Wire surface (revised)

```
POST /messages   { op: user_input{text} | interrupt | steer{text} | continue{text} }   append-to-log, coalesce
GET  /conversation                        snapshot of the message log (reconnect/rebuild)
GET  /events (SSE, one stream)            deltas: part.delta, part.updated, message.updated,
                                          run.status, wave.state, wave.idle, decision.pending|resolved
POST /decisions/{id} { approve|deny, comment }   idempotent
```
One SSE stream (not per-turn); additive delta patches so a late subscriber
rebuilds from the `GET /conversation` snapshot then resumes. An explicit
`wave.idle` / turn-done event is what the chat UI gates the composer on.

## The big fork (Codex surfaced — needs a call)

Our subagent is `codex exec --json`, **one-shot per pass**. That caps steer/
interrupt granularity at *between invocations* — coarse. Fine-grained steer +
interrupt (Codex's `turn/steer`, `turn/interrupt` with `expected_turn_id`) come
from driving a **long-lived `codex app-server` thread** with the SQ/EQ.

- **A — one-shot exec (today):** simple, less coupling; steer folds into the next
  pass; interrupt kills the pass. Coarse but honest.
- **B — long-lived codex thread:** fine mid-run steer/interrupt, but couples us
  to a moving vendor RPC and a persistent agent process.

Lean: **A now** (it matches the bounded-pass model and the reactive server), keep
B as the upgrade path if coarse steering proves too blunt.

## HumanLayer + 12-factor synthesis — the event-log spine

Four deep dives (daemon architecture, session lifecycle, CodeLayer UX, 12-factor)
converge on one structural move that unlocks the rest.

**Make the wave a fold over one append-only event log.** The log is the source
of truth; everything else is a projection:
- **Conversation (turns)** = a fold over the log (your "built from activity").
- **Memory** = a projection over the log; `MEMORY.md` is a human-readable view,
  not the racy mutable substrate (fixes the concurrent-write race we hit).
- **Run/wave/session status, cost, cursors** = a mutable *index*, rebuildable
  from the log (HumanLayer's split: `conversation_events` = truth, `sessions`
  row = cache).
- **Steering / subagent actions / human answers** = appended events.
- **Resume / fork / replay / durable-pause** all fall out for free (F6/F12).

This is the biggest single change and it's what #794 most lacks (today the
thread is in-memory, state is scattered across SQL + MEMORY.md + git + lost
codex transcripts — the exact F5 fragmentation 12-factor warns against).

### Adopt (server) — from hld, filtered for our per-wave (not central-daemon) model
- **Agent output = channel of typed stream events → one supervisor that appends
  to the log AND publishes to a bus.** Clean core; matches our supervisor.
- **Store = truth, bus = best-effort liveness (drop-on-full, non-blocking).**
  `GET /conversation` reads the log; SSE subscribes to the bus; client re-syncs
  from the log on reconnect. Keep the bus dumb.
- **`conversation_events` row shape** — `(id, session_id, sequence, event_type,
  role, content, tool_id, tool_name, tool_input, tool_result_for_id,
  is_completed, parent_tool_use_id)` — battle-tested normalization of an agent
  stream, incl. tool-call/result linkage. Steal near-verbatim.
- **Port 0 → print the actual port** for discovery (we already do `.wave-endpoint`
  — same idea, arguably more relevant to us than to them).
- **Orphan reconciliation on startup** — mark abandoned in-flight work failed.
- **Resume = fork a NEW row, never mutate** (`parent_*` set). Add an
  **`agent_thread_id`** to `Session` (their `claude_session_id` analogue) — the
  durable handle to the agent's own conversation; without it, resume can only
  cold-respawn. Keep **`run_id` as the durable pending-correlation key**.
- **Explicit `can_transition(from,to)` guards + a startup janitor** for stuck
  transient states — they lacked a guard table and got wedged in `interrupting`.

### Skip (central-daemon artifacts, not us)
- Two protocols (JSON-RPC socket + HTTP) — ship HTTP+SSE only.
- The many-sessions `SessionManager`/`activeProcesses` map — our server *is* one
  wave; don't build a manager-of-many and run one.
- `parent_session_id` chain-walking for conversation assembly — only needed if a
  logical thread fragments across processes; prefer one continuous log.
- Wide denormalized `sessions` row, 22+ ALTER migrations, socket-perms security
  (bind 127.0.0.1 + per-wave token instead).

### Adopt (Concerto WaveChat UX) — from CodeLayer, ranked
1. **Typed per-item rendering + a per-row status badge** (pending/running/done),
   not a generic message log. Diffs as real diffs; long tool output collapses to
   a preview + expand; plans as markdown that collapse on completion; a todo
   widget; subagent tool-calls grouped/collapsible.
2. **Inline decisions** — a gate renders *in the transcript at the turn it
   gates*, single-key approve/deny, deny-with-comment default. Not a separate
   inbox.
3. **Unified always-live composer** — running+empty = Interrupt; running+text =
   **Interrupt & Send** (redirect); waiting+text = **deny-with-your-text**; idle
   = send. One input collapses chat + interrupt + deny. (This is our steering UX.)
4. **Multi-wave: status = attention.** A dense keyboard-navigable table where the
   status cell pulls the eye; glyphs for autonomous vs gated waves; archive
   completed; OS notifications only on gate/attention. The human lives in the
   list, descends to WaveChat only to steer.

### Decisions this surfaces (yours)
- **SSE payload:** full message deltas (low latency, bus becomes load-bearing) vs
  change-notifications + re-fetch from `/conversation` (HumanLayer's conservative,
  resync-safe choice). Lean: notify + refetch, matching the store-is-truth split.
- **Gate frequency:** HumanLayer gates *every* risky tool call ("don't trust the
  agent"); waves are more autonomous. Adopt the inline-decision + composer
  mechanics wholesale, but make gating a **knob** (their auto-accept/bypass
  glyphs) rather than mandatory-per-tool.
- **Seed context (F3):** stop handing codex a raw `MEMORY.md` blob; render each
  pass's seed as tagged, token-dense sections (`<goal>`, `<wave_memory>`,
  `<recent_commits>`, `<last_failure>`, `<open_questions>`) folded from the log.
- **Human-contact as a typed event (F7):** replace side-channel talk-only chat
  with a `human_input_requested` event that durably pauses the run and resumes on
  a `response_from_human` event — turns chat from bystander into real steering.

## Concrete issues in #794 to fix regardless

- **Two status enums** — `ChatTurn.status: ChatTurnStatus` vs a separate
  `TurnStatus`; neither has `in_progress`. Collapse to one lifecycle enum.
- **DTO rule violations** — `ConversationItem` uses `#[serde(default)]` +
  `skip_serializing_if` (CLAUDE.md forbids `serde(default)` on wire types).
- **Rename** Conversation → WaveChat is agreed; carry it through.

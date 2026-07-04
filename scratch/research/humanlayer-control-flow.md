# HumanLayer: human-in-the-loop primitives (control flow)

Two generations, and the split matters:
- **Classic SDK** (`humanlayer` PyPI/npm) — stateless *tools-layer* lib: agent
  calls out to a human over Slack/email, blocks, resumes. Durable pending
  requests in HumanLayer's cloud.
- **Daemon (`hld`) + WUI / CodeLayer** — the current product (old top-level repo
  deprecated). A long-lived local server managing Claude Code sessions over HTTP +
  SSE, with an approval workflow and an event bus. Almost exactly our per-wave
  server.

## 1. Core primitives

**Classic SDK — "human as an async function":**
- `require_approval` — gates a specific tool call; blocks until approve or
  deny-with-feedback. Framework-agnostic.
- `human_as_tool()` — "ask a human" as a callable tool the LLM can choose.

Data model (spec + status pairs):
- `FunctionCall = { run_id, call_id, spec:{fn,kwargs,channel,reject_options},
  status:{requested_at,responded_at,approved,comment,user_info} }`
- `HumanContact = { run_id, call_id, spec:{msg,channel,response_options},
  status:{responded_at,response} }`
- `ContactChannel` — Slack/email/etc., routing + allowed responders.
- `ResponseOption = { name, title, description, prompt_fill }` — structured
  choices offered to the human.

Framing to steal: **a pending human decision is a first-class, addressable record
(`call_id` + spec + status), not an ephemeral prompt.** Resolution = filling in
the `status`.

**Daemon (`hld`) — session + approval model:**
- `Session = { ID, RunID, ClaudeSessionID, Status, Config, Result, StartTime,
  EndTime, Error }`.
- `Status` (9): `draft → starting → running → {waiting_input | interrupting →
  interrupted | completed | failed | discarded}`. Load-bearing: **`waiting_input`**
  (waiting for tool approval) and **`interrupted`** (can be resumed).
- `Approval = { approvalId, runId, toolName, toolInput, status, createdAt,
  comment }`. Agent requests via an auto-injected MCP tool
  (`mcp__codelayer__request_permission`); daemon persists an `approvals` row,
  flips session to `waiting_input`.

REST surface (daemon, default `localhost:7777`):
```
POST /sessions                      launch
GET  /sessions/{id}                 state
POST /sessions/{id}/continue        resume with new query + config overrides
POST /sessions/{id}/interrupt       stop (signal only, no body)
GET  /sessions/{id}/messages        conversation
POST /approvals                     create (runId, toolName, toolInput)
GET  /approvals?sessionId=          list pending
POST /approvals/{id}/decision       {decision: Approve|Deny, comment}  ← deny requires comment
GET  /events (SSE)                  event stream
```

## 2. Steering & interrupting — key finding

**No mid-run message-injection / steering endpoint in the daemon.**
`GetSessionMessages` reads; nothing writes into a *live* run. Steering is
**two-step, not a live channel**:
1. `POST /sessions/{id}/interrupt` → `interrupting` → `interrupted` (durable, resumable).
2. `POST /sessions/{id}/continue` with a **new `query`** (+ optional overrides:
   system prompt, allowed/disallowed tools, max_turns, mcp_config) → **forks a new
   session from the interrupted parent**, preserving context.

So HumanLayer's steering = **"interrupt, then continue with new instructions"** —
stop + re-enter, not injecting into the running loop. The narrower channel:
**deny-with-comment** on an approval bounces a specific tool call back with
feedback without killing the run. Decisions are idempotent (409 if already
decided).

## 3. Queueing / async / durable model
- **Durable pending requests.** Persisted rows (cloud `FunctionCall`/`HumanContact`
  classic; SQLite `approvals` + `sessions` daemon). The agent process can be gone;
  the request outlives it. (12-factor Factor 6: launch/pause/resume.)
- **Resume mechanics** (Factor 7): on `request_human_input`, `save_state(thread)`,
  notify, break the loop; a webhook (classic) or approval `decision` (daemon)
  arrives keyed by `run_id`/`call_id`, state reloads, execution resumes.
- **Push, not just poll.** SSE (`GET /events`, filter by Types/SessionID/RunID;
  30s heartbeats). Events: `new_approval`, `approval_resolved`,
  `session_status_changed`. Envelope: `{ Type, Data(map), Timestamp }`.

## 4. Recent direction — "outer loop" thesis
Dex Horthy (12-Factor Agents author) frames inner loop (agent's own
observe→act→check; AI owns it) vs outer loop (scheduling, bounded tasks, verify,
decide next, hard stops — historically human, increasingly managed). **Outer-loop
agents invert the trigger**: agent woken by cron/events, runs Agent→Human — it
reaches out when it needs a decision. HITL turns 80%-reliable autonomy into
shippable work. CodeLayer operationalizes it: full session visibility + approval
gates + interrupt/continue.

## What to adopt for the wave server
1. **Pending human decision = first-class durable record**, keyed to wave +
   `call_id`, spec/status shape, idempotent `POST .../decision` (409 on re-decide).
   Single most transferable idea — turns "chat" into "addressable decisions the
   wave is blocked on."
2. **A `waiting_input` wave state + `new_approval`/`approval_resolved`/
   `status_changed` SSE events.** Client renders "blocked on X" from state, not
   inferred from chat.
3. **`require_approval`-style gating on risky subagent tool calls** via an MCP
   permission tool; deny-with-comment feeds correction back without restarting.
4. **Steering = interrupt + continue** as the floor (durable, resumable
   `interrupted` + `continue(query, overrides)`). **Live mid-run injection is
   where we can beat them** — our agent loop is ours to instrument; HumanLayer
   deliberately only does stop-and-fork.
- **Skip** the Slack/email `ContactChannel`/`human_as_tool` router — our human is
  in the chat with the wave. Keep the `ResponseOption` idea (structured choices as
  buttons).

Sources: SDK + [Factor 7](https://github.com/humanlayer/12-factor-agents/blob/main/content/factor-07-contact-humans-with-tools.md),
[Factor 6](https://github.com/humanlayer/12-factor-agents/blob/main/content/factor-06-launch-pause-resume.md),
[Key Concepts / DeepWiki](https://deepwiki.com/humanlayer/humanlayer/1.1-key-concepts),
`hld/api/handlers/{sessions,approvals}.go`, `hld/PROTOCOL.md`, `hld/bus/events.go`.

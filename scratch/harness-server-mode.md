# Harness Server Mode

Wave: `lfd` · Item: `01-harness-server-mode.md`

## Problem

Today, every loopflow agent session is reachable in exactly one way: attach a terminal to its tmux session. That works for desktop Concerto and `tmux attach`. It does not work for iPhone Concerto, web clients, automation, or anything that can't render a PTY. The wave's three-plane architecture promised a separate **structured plane** for these clients; it doesn't exist yet.

The user this unblocks: a developer running waves on their dev machine (or a remote host) who wants to glance at progress, approve a destructive command, or answer a clarifying question from their phone. They don't want to look at terminal bytes. They want a structured timeline of tool calls, file edits, and questions, with first-class approve/answer gestures.

## Approach

Treat lfd as the **mediator** of the harness's own structured-interaction protocol, and re-export it on lfd's existing per-session SSE event stream + new POST endpoints. The wire format clients consume is **lfd's `SessionEvent` enum**, not the harness's native protocol — lfd normalizes across Codex/Claude/OpenCode so a mobile client learns one API.

Concretely:

1. **Extend `SessionEvent`** with `ApprovalRequested`, `ApprovalResolved`, `QuestionAsked`, `QuestionAnswered` variants. They are persisted and replayed on reconnect like every other session event.
2. **Stop auto-accepting in the Codex harness.** The auto-accept site (`harness/codex.rs:277-285`) becomes the routing point: emit an `ApprovalRequested` event, register the pending RPC `id` in a per-session pending-prompts table, and only respond to Codex once the client decides.
3. **Add HTTP endpoints** on the existing `/v1/sessions/{id}` route group:
   - `GET  /v1/sessions/{id}/pending` — list unresolved approvals/questions
   - `POST /v1/sessions/{id}/approvals/{prompt_id}` — `{ "decision": "accept" | "accept_for_session" | "decline" }`
   - `POST /v1/sessions/{id}/answers/{prompt_id}`  — `{ "text": "..." }`
   - `POST /v1/sessions/{id}/input`                — free-form user message into a running turn (Codex `turn/steer`)
4. **Codex first.** The protocol is JSON-RPC 2.0, officially documented and versioned ([developers.openai.com/codex/app-server](https://developers.openai.com/codex/app-server)), and lfd already speaks it. Approval + question round-trips fall out of unwiring auto-accept and adding the pending-prompts table.
5. **Claude second, in v1: documented auto-approve only.** Claude Code's stream-json input (`--input-format stream-json`) is undocumented at the CLI level (anthropics/claude-code#24594). Reverse-engineering the control protocol is fragile. v1 keeps Claude's existing `--skip-permissions` behaviour, ships a well-defined limitation in the API (`approval_supported: false` in the session capabilities), and lands the Claude approval flow as a follow-up using `claude-agent-sdk` — likely as an out-of-process Python helper that lfd manages.
6. **OpenCode: out of scope** for this item. Currently unused by any active wave. The same `SessionEvent` shape will accommodate it later via OpenCode's HTTP `POST /session/:id/permissions/:permissionID`.
7. **Reuse existing auth.** Bearer token middleware on `/v1/sessions/*` already exists. SSE replay via `after_seq` already exists.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Does the Codex app-server protocol support approval round-trips natively? | Yes. Server-initiated `item/commandExecution/requestApproval` and `item/fileChange/requestApproval` requests; client replies `{decision: accept \| acceptForSession \| decline \| cancel}` (developers.openai.com/codex/app-server). | Codex carries v1. Auto-accept becomes "forward to client." |
| Does Claude Code support a structured input channel for approvals? | `claude -p --input-format stream-json` accepts NDJSON on stdin, but the control-request schema (permissions, interrupts, set_permission_mode) is **undocumented** (anthropics/claude-code#24594). The Claude Agent SDK speaks it internally on the same channel. | Don't reverse-engineer in v1. Use auto-approve, expose `approval_supported: false`, plan SDK-proxy follow-up. |
| Does lfd already have a per-session structured event channel with replay? | Yes: SessionEvent enum is `#[non_exhaustive]` (`sessions/types.rs:271-341`); `list_events(session_id, after_seq)` (`sessions/mod.rs:416`); SSE route already exists. | New event variants slot in cleanly. Replay works for free. No WebSocket migration needed. |
| Where does Codex auto-accept live today? | `sessions/harness/codex.rs:277-285` — exactly one block, with the RPC `id` already in scope. | Single-site change to gate on a pending-prompts table. |
| Are SessionEvent additions wire-compatible? | `#[non_exhaustive]` enum with `#[serde(tag = "type", rename_all = "snake_case")]`. New variants serialize as new `type` values; old clients ignore unknown types. | Pure additive. No client break. |
| How long does Codex hold an unresolved approval? | Codex `app-server` keeps the request open indefinitely (no documented timeout); 30-min unsubscribe grace applies to the whole thread. | Pending prompts can hold across mobile-client gaps; we need a server-side TTL anyway to prevent unbounded growth. Pick 30 min to match Codex's thread grace. |
| Is Codex's `turn/steer` enough for "answer a clarifying question"? | Yes — it appends user content to the running turn. That's how the codex CLI itself handles mid-turn input. | `POST /v1/sessions/{id}/input` is a thin wrapper over `turn/steer` for active turns; for "between turns" it starts a new turn. |
| What about cost/budget visibility for mobile? | Claude already emits per-model cost in `result` events (cache_read/cache_creation broken out). Codex emits token counts in `turn.completed` but no $; we'd need a pricing table on our side. | Out of scope for this item — `TurnUsage` already exists. Mobile UX can render whatever's there; richer cost is its own item. |
| Output buffering pitfalls? | NDJSON over piped stdio is fine when each side flushes per line — all three CLIs do. Codex uses JSON-RPC over stdio, also line-flushed. No PTY needed. | Keep using piped stdin/stdout. No PTY allocation. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| **Pass-through harness protocols** (clients speak Codex JSON-RPC, Claude stream-json, OpenCode HTTP directly) | Zero normalization work in lfd; perfect fidelity | Mobile client must learn three protocols, and two are unstable/undocumented. Defeats lfd's value prop as runtime host. |
| **Adopt ACP (Zed's Agent Client Protocol)** | Vendor-neutral, future-friendly, OpenCode already implements it | Forces a wholesale shift in lfd's session model for marginal benefit today; ACP is young; we'd still translate from each harness. Revisit if ACP wins broadly. |
| **WebSocket bidirectional channel** instead of SSE+POST | One socket, lower latency for input | SSE+POST already works (sessions already use SSE), browsers/CDNs handle SSE well, mobile reconnect via `after_seq` already implemented. Not worth the migration. |
| **Implement Claude control protocol now** by reverse-engineering | Single delivery; no two-phase rollout | Protocol is undocumented (#24594); Anthropic could break us between minor versions. The SDK exists for a reason; spawn it. |
| **Build approvals on top of MCP** (Claude calls lfd's MCP server, MCP calls require approval) | Works today for Claude without control protocol | Only intercepts MCP-routed tools — built-in Bash/Edit/Write bypass it entirely, so the most dangerous calls aren't approvable. False sense of safety. |

## Key decisions

- **lfd's `SessionEvent` is the wire format.** Not Codex JSON-RPC, not Claude stream-json. lfd is the structured plane; harness protocols are implementation detail.
- **Codex carries v1; Claude is documented as auto-approve until we wire `claude-agent-sdk`.** Half-shipping a generic "approvals" API that silently fails on the most-used harness would be worse than a clearly-scoped first cut. Capability flag (`approval_supported: bool`) on the session DTO makes this explicit to clients.
- **SSE + POST, not WebSocket.** Existing infra; replay works; one less migration. Promote to WS only if a real client demands it.
- **Pending-prompts table is server-side state, persisted.** Approval requests survive lfd restarts and mobile-client gaps. TTL = 30 min (matches Codex thread grace), then auto-decline with `expired` reason.
- **No new auth surface.** Bearer token middleware on `/v1/sessions/*` covers everything. The wave doc confirms OAuth/tokens, not SSH, for structured clients.
- **`prompt_id` is lfd-issued, not the harness `rpc_id`.** Decouples wire format from Codex internals; lets us add Claude/OpenCode without reshaping the API.
- **Surface harness-specific approval flavors in the event payload.** Don't collapse Codex's `acceptForSession` / Claude's `allowedTools` / OpenCode's `remember` into a single boolean — store them as enum decisions and let advanced clients use them.

## Imagine wild success

Six months in: someone watching a wave on their phone gets a push notification — "Agent wants to run `rm -rf node_modules/.cache` in `~/src/foo`." They tap, see the command and the working directory, tap Approve. The desktop Concerto user is also looking; the approval moves to "resolved by mobile" in their UI in real time. Later that night, the same person scrolls through a session timeline of structured tool calls — bash command + exit code, file edit + diff, question + their own answer — that reads like a clean log, not a terminal scrollback. They never opened a terminal. The API is so clean that someone writes a Slack bot that posts approvals to a channel; another person writes a CLI that auto-declines anything outside `$REPO`. lfd became a programmable structured runtime, not just "the daemon behind Concerto."

## Imagine wild failure

Two scenarios to design against:

1. **The Claude protocol trap.** We get pressured into implementing Claude's control protocol natively in Rust to "match Codex." Anthropic ships Claude Code 2.2 and changes a field name. Sessions break in production for everyone using Claude (most users). We spend a sprint patching, then another sprint paying down what we should have started with: spawning `claude-agent-sdk`. *Mitigation: ship Claude as auto-approve in v1 with `approval_supported: false`. Make the limitation visible and the upgrade path obvious. Don't reverse-engineer.*

2. **Generic API, lossy reality.** We define `decision: "accept" | "decline"` as the API. It works for v1. iPhone Concerto ships. Then power users notice they can't say "accept for the rest of this session" via the app, even though Codex supports it natively. We add `accept_for_session`. Then "accept and remember as a rule" (OpenCode `remember`). Then `allowedTools` patterns (Claude). The API churns through three breaking versions in a quarter. *Mitigation: design `decision` as an enum carrying optional flavor up front; clients that don't understand a flavor fall back to `accept`. Pay the schema-design cost once.*

A third quieter failure: **the API has no consumer.** We ship the structured plane, iPhone Concerto slips, no other client picks it up, and the API drifts because nobody's exercising it. *Mitigation: pair this work with a minimal exerciser — at least an integration test that drives an end-to-end approval, and ideally desktop Concerto adopting the same SSE+POST shape so it has a real user immediately.*

## Scope

**In:**
- `SessionEvent` variants: `ApprovalRequested`, `ApprovalResolved`, `QuestionAsked`, `QuestionAnswered`. Persisted, replayed via `after_seq`, broadcast on existing per-session SSE.
- Per-session pending-prompts table (in-memory + persisted on lfd restart). 30-min TTL, auto-decline as `expired`.
- Codex harness changes: replace auto-accept with "emit ApprovalRequested, await client decision via channel, send response." Keep `accept` as the default if the session is configured for auto-approve (preserves current behavior for non-mediated runs).
- HTTP routes on `/v1/sessions/{id}`: `GET /pending`, `POST /approvals/{prompt_id}`, `POST /answers/{prompt_id}`, `POST /input`.
- Session DTO gains `approval_supported: bool` and `auto_approve: bool` capability flags.
- Integration test: spawn a Codex-backed session, trigger a tool call requiring approval, POST a decision, assert the round-trip and the session continues.
- API docs (one short README under `rust/loopflow/src/lfd/sessions/` covering the new endpoints and event types).

**Out:**
- Claude approval flow — v1 ships `approval_supported: false` for Claude with a tracked follow-up to wire `claude-agent-sdk`.
- OpenCode harness work — defer; same shape will fit when needed.
- WebSocket migration of session events.
- iPhone Concerto client itself (separate wave).
- Cost-budget enforcement on the API (`TurnUsage` already streams; that's enough for now).
- Push notifications — a delivery concern for the mobile app, not lfd.

## Done when

```bash
cargo test -p loopflow --test session_approval_round_trip
```

Specifically:

1. Spawn a Codex-backed session via lfd's HTTP API.
2. Send a turn that triggers a `requestApproval` from Codex.
3. Observe an `approval_requested` event on `GET /v1/sessions/{id}/events?after_seq=N` (SSE).
4. `GET /v1/sessions/{id}/pending` lists the prompt with the issued `prompt_id`.
5. `POST /v1/sessions/{id}/approvals/{prompt_id}` with `{"decision":"accept"}` returns 200.
6. Observe `approval_resolved` on the SSE stream; the session continues to `turn/completed`.
7. Disconnect the SSE subscriber for 10s mid-flow, reconnect with `after_seq`, receive every missed event including the approval pair.
8. Repeat with `{"decision":"decline"}`; assert the tool call is skipped and `turn/completed` carries the right status.

Manual smoke for the gap case:

```bash
# Claude session with auto-approve documented as a limitation
curl /v1/sessions  # response includes approval_supported: false for the claude session
```

— renders this as "agent runs autonomously" in the client UX rather than silently no-op'ing the approval button.

## Wave alignment

This item directly serves the wave's stated next step. From `wave/lfd/README.md`:

> The terminal plane is shipped; the structured plane is next... The interface for clients that can't be terminal participants (iPhone, web).

And from item 01:

> Non-terminal clients (iPhone Concerto) can interact with agent sessions through a structured API — observing output, responding to tool approvals and questions — without terminal access.

The design's "Done when" exercises exactly this finish line on Codex. The wave's core risk this design touches: **the structured plane drifts from the terminal plane.** Mitigated by reusing `SessionEvent` (already powering both planes' observation surface) and the existing per-session SSE infrastructure rather than introducing a parallel channel.

New risk this design introduces: **dependence on Codex `app-server` protocol stability.** Acceptable — the protocol is officially documented, JSON-Schema generated from Rust source, and OpenAI ships TS bindings from it. Versioned at the `initialize` handshake; we can pin and gate on version mismatch.

# Interactive agent sessions in lfd/Concerto (working design)

## User intent (verbatim)

> "I want to talk about building interactive steps into concerto in a more first place way."

> "The idea is that we should have be launching coding agents with HTTP sdks. OpenCode has a servermode, Cdoex has a sdk version, and claude has an undocumented --sdk-url option."

> "This will allow us to launch an interactive session and then without using some really messy PTY setup, be able to put a UI in Concerto for interacting with it."

> "The intention for Codex and Claude is to continue to use their built in Oauth."

> "We should try to create a unified agent sdk http api based on codex + opencode that we then translate as necessary into the three agents."

> "Study all three."

> "The concerto UI is less important to get right than just making sure were designing the API to evolve as the TUIs change their patterns."

> "The agent is launched by lfd whenever the the agent is support be in interactive mode."

> "Yes, the headless server should continue running until explicitly ended, and concerto can open/close as it likes."

> "Focused on pyramid-style design: lots of depth detail and production quality at the API/lfd level, just a very minimal slice at the Ui level to drive end-to-end testing to be expanded later."

## What we know from current code

- Interactive steps currently pause wave execution at `FlowAction::WaitInteractive`.
- `continue` today advances the run after auto-commit; it does **not** host an interactive SDK session.
- Existing output transport is line-oriented (`OutputHub`) + websocket events.
- There is placeholder PTY support in `lfd/sessions.rs` marked "future remote terminal support".

## Design direction (draft)

- Treat interactive agent execution as a durable **lfd-owned session runtime**, separate from Concerto process lifecycle.
- Introduce a **unified agent SDK protocol** inside lfd:
  - one canonical request/response/event model
  - provider adapters for Codex, Claude SDK URL bridge, and OpenCode server mode
- Keep v1 UI minimal (start session, stream transcript/events, send user input, resolve step).
- Prefer structured events over raw terminal bytes; keep fallback text path for unknown provider events.

## Open questions to resolve in this design session

1. Should a single waiting step allow multiple concurrent Concerto viewers with one input owner, or truly multi-writer?
2. Should we auto-resume in-progress sessions on lfd restart in v1, or mark them recoverable-but-reattach-required?
3. What is the canonical "step finished" signal: provider terminal event, explicit user action, or both?
4. Do we require provider capability negotiation (choice prompts, tool streaming, auth status), or infer from adapter type initially?


## Product study: Conductor + Sculptor (2026-02-21)

### Conductor patterns we should learn from

- **Workspace isolation as first-class unit**: one workspace per feature; multiple agents in parallel without interfering changes.
- **Provider login reuse**: Conductor reuses existing Claude/Codex auth on machine and supports provider env overrides.
- **Human control loop**: clear review/test/PR/archive flow in product, not just chat.
- **Checkpointing around turns (Claude-focused)**:
  - Stores turn snapshots separately from normal git history.
  - Uses Claude Code hooks + private git refs for reversible chat/code turns.
- **Operational pain signal**: FAQ calls out terminal stream corruption issues; this supports our move away from PTY-driven UX toward structured SDK events.

### Sculptor patterns we should learn from

- **Agent = isolated runtime + branch**: each task gets its own container, repo copy, and branch.
- **Long-lived session with detachable UI**: the runtime continues while user can jump across agents and reconnect.
- **Explicit sync model**: `Pull` (agent -> local) and `Push` (local -> agent), with conflict handling as first-class UX.
- **Pairing mode**: temporary local mirror of agent state; toggled on/off cleanly.
- **Provider capability matrix is explicit**:
  - Codex beta limitations are documented (no OpenAI OAuth in Sculptor beta; no MCP/custom config/model-family swap mid-session).
  - Product behavior degrades by capability instead of pretending parity.

### Implications for our lfd API design

1. **Session lifecycle must be independent of Concerto process**
   - create/start/attach/detach/terminate with durable state in lfd.

2. **Capability-negotiated protocol**
   - Each adapter advertises booleans/features (structured approvals, option prompts, interrupt, compact, auth-status, tool-stream events).
   - UI renders affordances from capabilities, not provider name.

3. **Structured interactive primitives over terminal bytes**
   - Canonical event types for: message deltas, tool lifecycle, approvals, single-choice/multi-choice/free-text prompts, status transitions, errors.
   - Keep text fallback path for unknown provider events.

4. **Turn snapshots/checkpoints in lfd**
   - Durable transcript + code-state anchors per turn to allow revert/replay/debug.
   - Provider-agnostic checkpoint API even if implementation differs by adapter.

5. **Sync semantics separated from conversation semantics**
   - Conversation events should not implicitly mutate step lifecycle.
   - Step completion should be explicit (provider-done + optional user-ack policy), not inferred from UI close.

### Additional clarifications from direct study of agent SDKs

- **Codex app-server** gives a strong model for typed turn/thread events + approval and user-input requests.
- **OpenCode server** gives a strong model for HTTP + SSE + explicit `question` and `permission` resources.
- **Claude** SDK URL path appears unstable/undocumented in local CLI; adapter should be guarded behind capability probing + fallback.


## Decisions confirmed with user (2026-02-21)

- Concerto flow in v1: **simple chat-style UI + explicit End button**.
- Milestone order: **Codex + Claude first**; OpenCode shapes protocol but is not required to be working in v1.
- Core protocol: **event stream model** (message/tool/status/input-request).
- Sessions: **resumable from Concerto reconnect/reopen**; lfd remains session host.
- Auth: rely on each provider's built-in OAuth/login; do not collect raw credentials in Concerto.
- Explicit non-goal v1: proving OpenCode adapter in production/testing.

## Proposed user flow (v1)

1. Wave reaches interactive step (`WaitInteractive`).
2. lfd creates `interactive_session` and immediately launches provider adapter for that step.
3. Concerto sees wave waiting + session status, opens a basic chat panel.
4. Concerto subscribes to session event stream (replay + follow).
5. User can submit free-text input; if provider emits structured prompt/options, UI renders buttons when possible, else fallback text box.
6. User clicks **End** when ready.
7. lfd sends `session.end` to adapter:
   - graceful stop if still running,
   - mark session ended,
   - execute existing wave continue behavior (auto-commit + step advance).
8. Session stays queryable for history/audit.

## Unified lfd protocol (draft)

### Core data structures (Rust sketch)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractiveProvider {
    Codex,
    Claude,
    OpenCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractiveSessionStatus {
    Starting,
    Running,
    WaitingForUser,
    Ending,
    Ended,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveSession {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub wave_run_id: LfdId,
    pub step: String,
    pub provider: InteractiveProvider,
    pub status: InteractiveSessionStatus,
    pub capability: SessionCapability,
    pub started_at: OffsetDateTime,
    pub ended_at: Option<OffsetDateTime>,
    pub provider_session_id: Option<String>,
    pub provider_metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionCapability {
    pub structured_input_requests: bool,
    pub option_prompts: bool,
    pub free_text_input: bool,
    pub tool_events: bool,
    pub interrupt: bool,
    pub auth_status_events: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InteractiveEvent {
    SessionStarted { session_id: LfdId },
    SessionStatus { status: InteractiveSessionStatus, detail: Option<String> },
    MessageDelta { role: String, content_delta: String },
    MessageFinal { role: String, content: String },
    ToolStarted { tool_call_id: String, tool_name: String },
    ToolDelta { tool_call_id: String, content_delta: String },
    ToolFinished { tool_call_id: String, ok: bool, summary: Option<String> },
    InputRequested { request: InputRequest },
    InputResolved { request_id: String },
    AuthStatus { state: String, detail: Option<String> },
    ProviderRaw { provider_type: String, payload: serde_json::Value },
    SessionEnded { reason: EndReason },
    SessionFailed { code: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputRequest {
    FreeText {
        request_id: String,
        prompt: String,
        placeholder: Option<String>,
    },
    SingleChoice {
        request_id: String,
        prompt: String,
        options: Vec<InputOption>,
        allow_free_text_fallback: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    UserEnded,
    ProviderCompleted,
    Cancelled,
    Error,
}
```

### Adapter interface (Rust sketch)

```rust
#[async_trait::async_trait]
pub trait InteractiveAdapter: Send + Sync {
    fn provider(&self) -> InteractiveProvider;

    async fn start(
        &self,
        req: StartSessionRequest,
        sink: InteractiveEventSink,
    ) -> Result<StartSessionResult, InteractiveAdapterError>;

    async fn send_input(
        &self,
        session: &InteractiveSession,
        input: UserInput,
    ) -> Result<(), InteractiveAdapterError>;

    async fn end(
        &self,
        session: &InteractiveSession,
        reason: EndReason,
    ) -> Result<(), InteractiveAdapterError>;

    async fn probe_capabilities(&self) -> SessionCapability;
}
```

## lfd HTTP API (draft)

### Session lifecycle

- `POST /waves/{wave_id}/interactive-sessions`
  - Usually internal call from executor when step enters interactive wait.
  - Returns session object.
- `GET /waves/{wave_id}/interactive-sessions/active`
  - Concerto reconnect entrypoint.
- `GET /interactive-sessions/{session_id}`
- `POST /interactive-sessions/{session_id}/end`
  - Body: `{ "reason": "user_ended" }`
  - On success, run existing continue/commit logic.

### User input

- `POST /interactive-sessions/{session_id}/input`
  - Body:
    - free text: `{ "kind": "free_text", "text": "..." }`
    - option pick: `{ "kind": "select", "request_id": "...", "option_id": "..." }`

### Event stream

- `GET /interactive-sessions/{session_id}/events`
  - SSE replay + follow (like chat + wave logs patterns).
  - Event payload is `InteractiveEvent`.

## Provider-specific adapter plan

### Codex (v1 primary)

- Prefer Codex app-server protocol as source of truth for typed interactive events.
- Map Codex approvals / requestUserInput into `InputRequested`.
- Map turn/thread lifecycle to session status transitions.

### Claude (v1 primary with guarded path)

- Attempt SDK URL adapter behind explicit capability probe.
- If unavailable, session creation fails fast with actionable error (`interactive_not_supported_for_current_claude`).
- Always keep free-text fallback for input flows.

### OpenCode (v1 design influence only)

- Protocol alignment only in v1 (question/permission resources influence our `InputRequest` shape).
- Implementation deferred to later milestone.

## Persistence/resume model

- Add durable store records:
  - `interactive_sessions`
  - `interactive_events`
  - optional `interactive_requests` (open input requests)
- Concerto reopen behavior:
  - call `.../active`
  - attach stream from last seen event cursor
  - continue interaction if status is running/waiting-for-user
- lfd restart behavior (v1):
  - persisted history remains readable,
  - active adapter process recovery is best-effort only unless provider supports reconnect token.

## Edge cases to handle early

1. **Choice prompts** ("choose 1 of 4")
   - emit typed `SingleChoice` request + optional free-text fallback.
2. **Unknown provider event**
   - forward as `ProviderRaw` so UI/logs never go blind.
3. **UI disconnect/reconnect**
   - no effect on session runtime; stream is replayable.
4. **Double-end clicks / concurrent clients**
   - idempotent end endpoint; first win sets terminal status.
5. **Provider auth interruption**
   - emit `AuthStatus` event; keep session alive when possible.
6. **Wave state mismatch**
   - session can only end/continue if wave run still waiting on that session.

## Minimal Concerto slice (v1)

- Replace embedded PTY/Ghostty session path for interactive steps with HTTP session APIs.
- Render:
  - transcript list,
  - free-text composer,
  - optional choice buttons for `SingleChoice`,
  - `End` button.
- Do not build advanced layout/tool UIs yet.

## Verification (v1)

- Integration test: interactive wave run creates session and blocks step progression.
- Integration test: posting input emits mapped provider event and persists transcript.
- Integration test: end session advances wave run to next step.
- Reconnect test: Concerto can close/reopen and reattach to active session.
- Contract tests: adapter event mapping (Codex + Claude) into `InteractiveEvent`.

## Size-check

This design is likely **larger than a single commit**:

- New persistent store tables + migrations
- New lfd runtime manager + adapter abstraction
- New HTTP routes + SSE protocol + tests
- Codex adapter + Claude adapter
- Concerto minimal UI rewrite from PTY terminal to HTTP chat stream

Estimated implementation: well over ~1000 LOC and likely multiple shippable milestones.

Recommendation: split into a **wave plan** with stage 1 as a thin but production-quality lfd protocol spine.

## Proposed wave stages (draft)

1. **Stage 1 — Protocol spine + fake adapter**
   - Add session/event domain types, storage, routes, SSE replay/follow.
   - Add deterministic fake adapter for end-to-end tests.
   - Concerto minimal chat + End button against new API.

2. **Stage 2 — Codex adapter**
   - Wire Codex app-server events into unified protocol.
   - Handle input requests/options/approvals.

3. **Stage 3 — Claude adapter (guarded)**
   - Implement SDK URL bridge with capability probing and explicit unsupported errors.

4. **Stage 4 — OpenCode adapter**
   - Implement + test OpenCode server mode integration.


## API comparison: structured events vs PTY (Codex vs OpenCode)

### Why this beats PTY scraping

- PTY gives bytes + terminal control sequences; semantics (approval, option prompt, tool lifecycle) are implicit and brittle.
- Codex/OpenCode both expose explicit machine-readable primitives for session/turn progression and user-input interrupts.

### Codex app-server shape

- Transport: bidirectional JSON-RPC over stdio (supported) and websocket (marked experimental/unsupported).
- Core primitives: thread -> turn -> item lifecycle.
- Streaming: `turn/started`, `item/*`, deltas, `turn/completed` notifications.
- Interactive interrupts: explicit approval and `tool/requestUserInput` flows where server pauses turn until client responds.

### OpenCode server shape

- Transport: HTTP API with OpenAPI spec + SSE event stream (`/event`, `/global/event`).
- Core primitives: session/message resources with sync and async prompt endpoints.
- Interactive interrupts: explicit permission response endpoint (`/session/:id/permissions/:permissionID`) plus event stream updates.
- Auth endpoints are explicit (provider OAuth authorize/callback routes).

### Direct comparison for our unified API

- Codex is richer in **native interactive turn protocol** (server-initiated requests).
- OpenCode is cleaner in **HTTP resource modeling** (session-centric REST + SSE).
- Best hybrid for lfd:
  - external API: OpenCode-like HTTP/SSE shape (stable for Concerto + tests)
  - adapter internals: Codex-style typed turn/item event mapping
  - canonical `input_request` abstraction to represent approvals + option prompts + free text

### Adapter complexity impact

- Codex adapter: lower semantic impedance (already structured around turn/item/approvals).
- OpenCode adapter: low impedance on transport (HTTP) and permissions; event taxonomy maps well.
- PTY fallback: highest maintenance risk; should be last-resort capability mode only.


## New constraint from user experience (2026-02-21)

> "my experience is that oneshot/-p is going to lead to very different agent behavior than running interactive"

Design impact:

- Do **not** treat `claude --print` turn-chaining as equivalent to interactive mode.
- Claude v1 adapter should target a **native interactive session transport** (SDK URL or equivalent) to preserve behavior.
- If native interactive transport is unavailable, fail with explicit capability error rather than silently degrading to one-shot semantics.
- One-shot headless mode remains valid for non-interactive steps only.


## Claude uncertainty plan: what to detect next

We should not decide PTY vs SDK by guesswork. Run a capability probe matrix and gate implementation on results.

### Immediate signal from local binary

- `claude --definitely-not-a-real-flag -p "say hi"` returns unknown option.
- `claude --sdk-url http://127.0.0.1:9 -p "say hi"` did **not** fail as unknown option.

Inference: `--sdk-url` is recognized by this installed binary, but protocol/behavior is still unknown.

### Probe matrix (ordered)

1. **Transport shape probe**
   - Point `--sdk-url` at a local capture server.
   - Detect: HTTP vs WS, request paths, auth headers, event framing.

2. **Interactive semantics probe**
   - Run same scripted prompt in:
     - native Claude TUI
     - Claude via `--sdk-url`
   - Compare event traces for: tool lifecycle, interrupts, choice prompts, completion boundaries.

3. **Input-request fidelity probe**
   - Force scenarios like "choose one option" and permission prompts.
   - Confirm whether sdk transport emits typed request objects vs plain text instructions.

4. **Session continuity probe**
   - Start session, disconnect client, reconnect, continue turn.
   - Validate stable provider session id / resume token behavior.

5. **Auth ownership probe**
   - Ensure OAuth/login stays within Claude process and no raw credentials transit Concerto/lfd.

### Decision gate

- If probes 1-4 pass with acceptable parity: Claude adapter = SDK transport (no PTY).
- If transport works but choice/approval fidelity is weak: keep Claude in gated beta with explicit limitations.
- If transport fails parity: return explicit `interactive_not_supported_for_claude` (preferred) or choose PTY fallback by explicit product decision.


## Probe findings (Claude SDK URL) — 2026-02-21

### Confirmed

1. `--sdk-url` is accepted by this Claude binary (not rejected as unknown option).
2. Hidden CLI text in `@anthropic-ai/claude-agent-sdk` says:
   - `--sdk-url <url>` is for **remote WebSocket endpoint for SDK I/O streaming**
   - intended only with `-p` + `--input-format stream-json` + `--output-format stream-json`.
3. With `ws://127.0.0.1:<port>`, Claude opens a WebSocket connection (`GET /` + Upgrade headers).
4. Once connected, Claude sends ping frames every ~10s (opcode 9), and accepts pong.
5. We did **not** observe application data frames from Claude yet (only keepalive ping).
6. With `http://127.0.0.1:<port>`, local HTTP capture saw no useful app traffic from our probes.
7. `lsof` while running `--sdk-url` shows Claude still talking to remote `:443` endpoints (auth/model backend remains inside Claude process).
8. Debug log includes: `Fast mode unavailable: Fast mode is not available in the Agent SDK`.

### Reverse-engineered from bundled JS

- Class `jU8` (sdk-url transport) extends line-based stream handler (`ol6`).
- Incoming WebSocket data is fed into a newline-delimited JSON input stream.
- Outgoing SDK messages are serialized and written over WebSocket transport.
- Initial prompt (if provided with `-p`) is injected into local input stream as a `type:"user"` JSON line.

### Current unknown

- Required initial message sequence for successful Claude <-> remote SDK session (we have not yet triggered non-ping application frames).
- Whether additional handshake/auth/session bootstrap messages are required from server side before Claude emits events.


### Print mode behavior observations (from direct run)

- `claude -p --input-format stream-json --output-format stream-json --verbose` returns a structured stream with:
  1) `system/init`
  2) `assistant`
  3) `result`
- This confirms print mode runs through the SDK/headless event contract and emits a terminal `result` envelope.

Implication:
- `--sdk-url` is a transport override for that headless SDK contract; it does not by itself switch Claude back to TUI-style interactive orchestration.


## Provisional architecture decision (pending final fork choice)

- Claude v1: use an lfd-owned PTY adapter that translates interactive terminal I/O into unified session events.
- Keep OAuth/auth ownership in Claude CLI.
- Keep public lfd API provider-agnostic (`/interactive-sessions/*` + SSE events).
- Preserve upgrade path: if SDK URL reaches behavioral parity later, swap adapter internals without changing API/UI.


## Detailed design: items 1-3 owned by implementation research

### 1) Claude PTY adapter event contract (guaranteed vs best-effort)

#### Guaranteed events (must always work)

- `session_status`
  - transitions for `starting/running/waiting_for_user/ending/ended/failed`
- `message_delta`
  - incremental assistant text stream when detectable
- `message_final`
  - flushed assistant chunk at stable boundary (line/turn/end)
- `raw_output`
  - unparsed terminal output passthrough for safety/debug
- `session_end` / `session_failed`
  - explicit terminal state

#### Best-effort events (capability-gated)

- `input_request`
  - `single_choice` and `approval` inferred from terminal patterns
- `tool_started/tool_delta/tool_finished`
  - only when parser confidence exceeds threshold

#### Capability advertisement for Claude PTY v1

```json
{
  "structured_input_requests": "partial",
  "option_prompts": "partial",
  "free_text_input": true,
  "tool_events": "partial",
  "interrupt": true,
  "raw_output_fallback": true
}
```

### 2) Input-request detection strategy ("choose 1 of 4" and approvals)

#### Tiered parser pipeline

1. **Tier A: explicit structured markers**
   - If Claude emits recognizable structured prefixes/markers, map directly.
2. **Tier B: deterministic rule engine**
   - Versioned regex/rule set keyed by adapter version.
   - Examples:
     - numbered options list + imperative selection sentence -> `single_choice`
     - permission-style question + approve/deny language -> `approval`
3. **Tier C: raw fallback**
   - No confident parse -> emit `raw_output` only and keep free-text input enabled.

#### Safety policy

- Parser never blocks the user.
- False negatives are acceptable; false positives should be minimized.
- All inferred events include `confidence` metadata for UI/telemetry.

### 3) Session lifecycle/state machine (resumable)

```text
starting -> running -> waiting_for_user (optional) -> ending -> ended
                                   \-> failed
```

#### State semantics

- `starting`: adapter process spawn + stream attach in progress
- `running`: normal bidirectional interaction
- `waiting_for_user`: explicit or inferred pause requiring user input
- `ending`: idempotent end requested; adapter shutdown in progress
- `ended`: graceful completion
- `failed`: unrecoverable adapter/process error

#### Lifecycle invariants

- `end` is idempotent (multiple calls safe).
- UI disconnect does not change session state.
- Reconnect replays persisted events then follows live stream.
- Only one active interactive session per waiting wave step.
- Session end can trigger wave continue only when wave run still waits on that session id.


## Naming refinement: prefer `agent`, avoid ambiguous `session`

Agreed: `session` is overloaded, and we should use `agent` where possible.

### Proposed canonical naming

- **`agent`** = first-class runtime unit (interactive lifecycle owned by lfd)
- **`provider_session_id`** (or `provider_conversation_id`) = external provider resume handle (e.g., Claude `--resume` id)
- **`wave_run`** remains orchestration unit that may be waiting on an `agent`

### Parent/child model (agreed direction)

- `wave_runs` is the orchestration parent.
- `agents` is the execution child (`agents.wave_run_id`).
- `agent_events` is child of `agents` (`agent_events.agent_id`).

Cardinality:
- `wave_run` → many `agents` (retries, relaunches, provider switches over time)
- `agent` → many `agent_events`

Operational rule for interactive steps:
- at most one **active** interactive agent per wave_run at a time (history can contain multiple ended agents).

### Table/API naming proposal

- DB table: keep existing `agents` table (no physical rename in this wave)
- Event table: `agent_events` (required for interactive history/replay)
- API (agent-centric):
  - `POST /waves/{wave_id}/agents`
  - `GET /waves/{wave_id}/agents/active`
  - `GET /agents/{agent_id}`
  - `POST /agents/{agent_id}/input`
  - `POST /agents/{agent_id}/end`
  - `GET /agents/{agent_id}/events`

### Recovery metadata (required for Claude resume)

Persist on `agents`:
- `provider` (`claude|codex|opencode|fake`)
- `provider_session_id` (resume token/id)
- `wave_run_id`, `step`, `worktree`, `cwd`
- launch options (model/permission mode/flags)
- status timestamps

This keeps naming explicit and avoids conflating provider conversation state with lfd runtime state.

### New requirement from user: persistent event history + harness-aligned models

> "we will need some sort of persistent on the session events to be able to see the chat history, and well want to use similar setups/tables/schemas/models that we do in the harness project"

Implications:
- `agent_events` is **required** for interactive runs.
- Persist full structured events, not only flattened chat text rows.
- Align event envelope/types with harness `AgentEvent` concepts:
  - `message`
  - `tool_call`
  - `tool_result`
  - `memory_edit`
  - `done`
  - `failed`
- UI chat history should be renderable from `agent_events` replay, with `chat_messages` treated as legacy/non-interactive history or a derived read model.

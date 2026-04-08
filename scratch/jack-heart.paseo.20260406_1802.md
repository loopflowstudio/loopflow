# Paseo vs Loopflow: codebase comparison

Based on a local checkout of `https://github.com/getpaseo/paseo` at commit `03380cfad0298df28847f9aea26bcc897d0213de` (`docs(changelog): make 0.1.49 release notes user-friendly`, 2026-04-07 +0700).

## Bottom line

Paseo is an **agent runtime + control plane**.
Loopflow is a **workflow engine + shipping system**.

They overlap in daemon/session/provider infrastructure, but they optimize for different layers:

- **Paseo**: run agents, stream them, coordinate them, inspect them, control them from anywhere.
- **Loopflow**: decide what work matters, shape how it flows, persist project intent, and land it cleanly.

The interesting conclusion is not “they are ahead” or “we are ahead.”
It’s that they are ahead on the **moment-to-moment operating system for agents**, while we are ahead on the **campaign-level system for getting software shipped**.

## Quick framing

### Loopflow’s unit of work

Wave = **area × direction × flow**.

Evidence:
- `README.md`
- `rust/loopflow/src/engine/flow.rs`
- `rust/loopflow/src/engine/prompt.rs`
- `rust/loopflow/src/lfd/triggers/*`
- `rust/loopflow/src/ops/land.rs`

### Paseo’s unit of work

Agent session inside a workspace, with optional higher-level helpers for chat, loops, schedules, and worktrees.

Evidence:
- `docs/ARCHITECTURE.md`
- `packages/server/src/server/agent/agent-sdk-types.ts`
- `packages/server/src/server/session.ts`
- `packages/server/src/server/loop-service.ts`
- `packages/server/src/server/chat/chat-service.ts`
- `packages/server/src/server/schedule/service.ts`

## Similarities

These are genuinely close:

### 1. Both are daemon-first systems

- **Loopflow**: `lfd` is the center; HTTP + websocket-ish event streaming + scheduler + storage.
  - `docs/lfd.md`
  - `rust/loopflow/src/lfd/http/mod.rs`
  - `rust/loopflow/src/lfd/events.rs`
- **Paseo**: the Node daemon is the center; WS protocol + agent lifecycle + relay + storage.
  - `docs/ARCHITECTURE.md`
  - `packages/server/src/server/bootstrap.ts`
  - `packages/server/src/server/websocket-server.ts`

### 2. Both use git worktrees as the isolation primitive

- **Loopflow**: sibling worktrees are load-bearing and tied to wave identity.
  - `rust/loopflow/src/engine/worktrees.rs`
  - `docs/lfop.md`
- **Paseo**: managed worktrees under `~/.paseo/worktrees/{hash}/{slug}` with setup/teardown hooks.
  - `packages/server/src/utils/worktree.ts`

### 3. Both assume multiple providers matter

- **Loopflow**: Claude / Codex / OpenCode support exists, but the abstraction is thinner and prompt-centric.
- **Paseo**: provider abstraction is a first-class product surface.
  - `packages/server/src/server/agent/provider-registry.ts`
  - `packages/server/src/server/agent/provider-manifest.ts`
  - `packages/server/src/server/agent/agent-sdk-types.ts`

### 4. Both are explicitly building for parallel agent work

- **Loopflow**: waves, forks, flow DAGs, garden governance.
  - `rust/loopflow/src/engine/flow.rs`
- **Paseo**: multiple agent panes, chat rooms, agent MCP management, loops, schedules.
  - `packages/server/src/server/agent/agent-management-mcp.ts`
  - `packages/server/src/server/chat/chat-service.ts`
  - `packages/server/src/server/loop-service.ts`

## The big difference

### Loopflow: strategic orchestration

Loopflow has strong opinions about:
- what work should exist (`wave/`)
- how work should flow (flows, XORs, loops, ops)
- what perspective shapes quality (directions)
- how and when work ships (`lf op pr`, `lf op land`, PM sync, merge queue)

This shows up in:
- `rust/loopflow/src/engine/flow.rs`
- `rust/loopflow/src/engine/prompt.rs`
- `rust/loopflow/src/lfd/triggers/*`
- `rust/loopflow/src/ops/*`
- `README.md`, `docs/wave-authoring.md`, `docs/lfop.md`

### Paseo: operational orchestration

Paseo has strong opinions about:
- how to run a provider session reliably
- how to normalize telemetry across providers
- how to reconnect a client without losing the thread
- how to drive agents from mobile/web/desktop/CLI
- how to coordinate agents ad hoc
k 
This shows up in:
- `packages/server/src/server/agent/agent-sdk-types.ts`
- `packages/server/src/server/agent/agent-manager.ts`
- `packages/server/src/server/agent/timeline-projection.ts`
- `packages/app/src/contexts/session-context.tsx`
- `packages/app/src/contexts/session-timeline-seq-gate.ts`
- `packages/relay/src/*`

## Where Paseo is ahead

## 1. Provider normalization is much more mature

This is their cleanest win.

They have a real provider interface:
- `AgentClient`
- `AgentSession`
- capability flags
- model definitions
- persistence handles
- permission request/response types
- normalized tool-call detail

File:
- `packages/server/src/server/agent/agent-sdk-types.ts`

The `ToolCallDetail` union is especially valuable:
- `shell`
- `read`
- `edit`
- `write`
- `search`
- `fetch`
- `worktree_setup`
- `sub_agent`
- `plan`
- `plain_text`
- `unknown`

Then each provider maps into that format:
- Claude parser: `packages/server/src/server/agent/providers/claude/tool-call-detail-parser.ts`
- Codex mapper: `packages/server/src/server/agent/providers/codex/tool-call-mapper.ts`

### What this buys them

- same UI for different providers
- same timeline model for different providers
- same permission surface for different providers
- same stream reducers in the app

### What we should learn

We need a **canonical internal tool/activity event model** sooner rather than later.

Not because it’s pretty. Because Concerto and any future multi-provider UI will otherwise keep learning provider quirks at the edge.

**Concrete steal:**
- a Loopflow equivalent of `ToolCallDetail`
- a Loopflow equivalent of `AgentCapabilityFlags`
- provider adapters that emit normalized activity instead of raw provider-specific chunks

Relevant Paseo files:
- `packages/server/src/server/agent/agent-sdk-types.ts`
- `packages/server/src/server/agent/providers/codex/tool-call-mapper.ts`
- `packages/server/src/server/agent/providers/claude/tool-call-detail-parser.ts`
- `packages/server/src/server/agent/activity-curator.ts`

## 2. Their stream/timeline model is stronger than ours

Paseo has done serious work around stream correctness.

Server side:
- per-item `epoch` + `seq`
- projected vs canonical timelines
- tool lifecycle collapse
- assistant chunk merging
- timeline window fetch with overlap/gap handling

Files:
- `packages/server/src/server/agent/timeline-projection.ts`
- `packages/server/src/server/agent/agent-manager.ts`
- `packages/server/src/shared/messages.ts`

Client side:
- sequence gate that classifies `accept | drop_stale | drop_epoch | gap | init`
- head/tail stream model
- catch-up fetch on resume
- dedupe and gap handling in UI state

Files:
- `packages/app/src/contexts/session-timeline-seq-gate.ts`
- `packages/app/src/contexts/session-context.tsx`
- `packages/app/src/types/stream.ts`

Loopflow’s current event hub is intentionally much simpler:
- `rust/loopflow/src/lfd/events.rs`

That simplicity is nice, but once Concerto wants robust reconnect, partial history, timeline compaction, and multiple concurrent observers, Paseo’s design is the more mature template.

### Concrete steal

Not their entire architecture. Just the hard-won pieces:
- cursor = `{epoch, seq}`
- stale/duplicate/gap classification
- canonical vs projected timeline views
- provider-normalized tool lifecycle collapsing

## 3. Remote access is real, and the relay design is worth studying

Paseo’s relay is not hand-wavy.

Files:
- `packages/relay/src/crypto.ts`
- `packages/relay/src/encrypted-channel.ts`
- `packages/relay/src/cloudflare-adapter.ts`
- `SECURITY.md`

What they implemented:
- persistent daemon keypair
- QR/pairing flow
- ECDH shared secret
- NaCl box crypto
- encrypted channel abstraction
- Cloudflare Durable Object relay with server-control + per-connection data channels
- buffering + reconnect nudging

This is much more mature than “someday remote.”

### What we should learn

When Loopflow wants remote `lfd` access, this is the right family of design.
Not necessarily the exact code, but the structure is solid:
- outbound daemon connection
- untrusted relay
- pairing as trust anchor
- channel-level reconnect semantics

## 4. Cross-device product surface is real, not aspirational

Paseo has:
- Expo app: `packages/app`
- Electron desktop: `packages/desktop`
- CLI: `packages/cli`
- web/website: `packages/website`

This matters because it forced them to make the daemon protocol explicit.
A lot of design quality falls out of that pressure.

Loopflow has a better conceptual model, but Paseo has done more work on the **client protocol contract**.

## 5. Chat rooms are a lightweight coordination primitive we do not have

File:
- `packages/server/src/server/chat/chat-service.ts`

This is simple, but interesting:
- create room
- post messages
- mention agents with `@agent-id`
- wait for new messages
- persistent room/message store

The implementation is file-backed JSON, so it’s not sophisticated infrastructure.
But the primitive itself is useful.

Loopflow currently coordinates mostly through:
- flow structure
- scratch/
- wave state
- triggers

That gives us stronger structure, but less ad hoc collaboration.

### Concrete steal

Not necessarily “chat rooms” as the product feature.
More likely: a **shared coordination channel** for humans + agents + waves.

This could complement garden/scan/assess/mutate rather than replace it.

## 6. Worktree lifecycle hooks are richer than ours

File:
- `packages/server/src/utils/worktree.ts`

Paseo supports:
- managed worktree locations under `~/.paseo/worktrees/{hash}/{slug}`
- setup hooks from `paseo.json`
- teardown hooks
- runtime env injection (`PASEO_WORKTREE_PORT`, source checkout path, etc.)
- terminal specs per worktree
- cleanup on failed setup

Loopflow’s worktree system is stronger at naming and wave semantics, but weaker as a **runtime bootstrap surface**.

### Concrete steal

A repo-local worktree hook model would be useful.
Not the storage path convention. The lifecycle hooks.

## Where Loopflow is ahead

## 1. Loopflow has the real orchestration model

Paseo has loops, schedules, chat, and manual orchestration helpers.
Loopflow has:
- waves
- directions
- flows as DAGs
- XOR routing
- triggers
- crons tied to waves
- meta-waves/garden governance
- PM ingestion/export

Paseo has nothing equivalent to:
- `wave/`
- area scoping
- direction stacking
- `garden/scan -> garden/assess -> wave/mutate`
- `lf op pm *`

That is a big difference.

Paseo helps you operate agents.
Loopflow helps you operate **work**.

## 2. Loopflow’s prompt/context assembly is much deeper

Loopflow’s prompt system is one of its strongest unfair advantages.

Files:
- `rust/loopflow/src/engine/prompt.rs`
- `rust/loopflow/src/engine/launch.rs`
- `README.md`

What we do that Paseo does not:
- area docs
- repo docs
- wave docs
- wave memory
- directions
- diff assembly
- flow-aware prompt shaping
- worktree-aware wave identity

Paseo mostly passes agent config + prompt.
Loopflow assembles a whole operating context.

## 3. Loopflow has shipping mechanics; Paseo mostly stops at execution

Loopflow:
- PR creation
- merge queue land
- next branch flow
- combine/split/abandon patterns
- PM push-diff
- release workflows

Files:
- `rust/loopflow/src/ops/*`
- `docs/lfop.md`

Paseo has some git/worktree affordances, but not the same shipping system.
No equivalent to `lf op land` as a core path.

## 4. Loopflow storage/backend architecture is more serious

Loopflow:
- SQLite/Postgres backend
- migrations
- typed store layer
- durable wave/session/trigger/queue state

Files:
- `rust/loopflow/src/lfd/store/mod.rs`
- `rust/loopflow/src/lfd/store/sqlite.rs`
- `rust/loopflow/src/lfd/store/postgres.rs`
- `rust/loopflow/src/lfd/store/migrations.rs`

Paseo persists critical state to JSON files in multiple places:
- agents
- chat rooms/messages
- loops
- schedules
- workspace-ish state

Examples:
- `packages/server/src/server/agent/agent-storage.ts`
- `packages/server/src/server/chat/chat-service.ts`
- `packages/server/src/server/loop-service.ts`
- `packages/server/src/server/schedule/store.ts`

That is fine for a local-first tool at this stage, but it is not the same durability/concurrency story.

## Feature comparison

| Area | Paseo | Loopflow | Verdict |
|---|---|---|---|
| Multi-provider runtime | Strong | Medium | Paseo ahead |
| Tool-call normalization | Strong | Weak/medium | Paseo ahead |
| Timeline/reconnect semantics | Strong | Medium | Paseo ahead |
| Remote relay | Strong | Little/no productized support | Paseo ahead |
| Mobile/web/desktop clients | Strong | Concerto macOS only | Paseo ahead |
| Voice stack | Strong | Not a priority | Paseo ahead for scope |
| Worktree semantics for campaign management | Medium | Strong | Loopflow ahead |
| Workflow DAGs / XOR / loops | Weak/medium | Strong | Loopflow ahead |
| Meta-orchestration / garden | Weak | Strong | Loopflow ahead |
| Prompt/context assembly | Medium | Strong | Loopflow ahead |
| PM integration | Weak | Strong | Loopflow ahead |
| PR / merge queue shipping | Weak | Strong | Loopflow ahead |
| Storage/backend rigor | Medium | Strong | Loopflow ahead |

## Implementation quality assessment

## What Paseo implements well

### Strongly implemented

1. **Provider abstraction**
   - cohesive types
   - explicit capability flags
   - good separation between registry/manifest/provider implementations

2. **Timeline normalization and UI streaming**
   - server and app clearly shaped around real stream edge cases
   - lots of tests around seq/gap/tool-call state

3. **Relay**
   - credible security model
   - practical reconnect/channel behavior

4. **Worktree utility layer**
   - not philosophically aligned with Loopflow’s sibling convention, but pragmatically solid
   - setup/teardown/runtime env is useful and concrete

5. **Test culture**
   - large test surface: ~336 test files across the repo, ~165 in server alone
   - many integration and e2e tests around hard runtime edges

## Mixed quality

### Chat service

Useful feature, but implementation is basic.

- `packages/server/src/server/chat/chat-service.ts`

Observations:
- file-backed JSON store
- in-memory waiter tracking
- simple persistence queue
- fine for local-first, less convincing for long-term concurrency/load

### Loop service

Useful, but not especially deep.

- `packages/server/src/server/loop-service.ts`

It is basically:
- worker agent runs
- shell verify checks run
- verifier agent judges pass/fail
- sleep and repeat

That is a respectable Ralph-loop implementation.
But it is nowhere near Loopflow’s flow engine.

### Schedule service

Again: useful, but straightforward.

- `packages/server/src/server/schedule/service.ts`

It schedules prompts against either:
- an existing agent
- a new agent config

This is a scheduling primitive, not a full planning/orchestration system.

## Weak spots / architectural debt

### 1. `session.ts` is a god object

- `packages/server/src/server/session.ts` is **8202 LOC**.

That file is doing too much:
- per-client session state
- request routing
- workspace handling
- timeline fetching
- worktree operations
- permission handling
- file explorer
- relay-adjacent behavior
- loop/chat/schedule integration glue

This is the clearest “they are moving fast, but this wants refactoring” signal in the codebase.

### 2. Some docs lag the code

Example:
- `packages/server/README.md` still reads like an older “voice assistant” package README, not the current daemon reality.

That usually means the code evolved faster than the docs.

### 3. File-backed persistence is going to pinch

Current design is acceptable for local-first desktop tooling.
But it will become awkward around:
- concurrent writers
- richer history queries
- indexing/search
- data migrations
- partial corruption recovery

Loopflow’s DB-backed path is stronger here.

### 4. Orchestration as product is still thinner than orchestration as runtime

Interesting tell:
- `packages/server/src/server/agent/orchestrator.ts` is just a deprecated stub saying the logic moved into `session.ts`.

That fits the overall picture: they have many orchestration-adjacent features, but the **conceptual center** is still the session runtime, not a distinct orchestration model.

## What code we should look at adding

## Steal now

### 1. Canonical tool/activity model

Target idea:
- Loopflow-internal `ToolCallDetail` / `ActivityDetail` enum/union
- provider adapters emit normalized activity
- Concerto consumes one schema

Paseo files to study:
- `packages/server/src/server/agent/agent-sdk-types.ts`
- `packages/server/src/server/agent/providers/codex/tool-call-mapper.ts`
- `packages/server/src/server/agent/providers/claude/tool-call-detail-parser.ts`
- `packages/server/src/server/agent/activity-curator.ts`

Likely Loopflow landing zone:
- `rust/loopflow/src/lfd/sessions/*`
- `rust/loopflow/src/lfd/types/*`
- `swift/LoopflowCore/Services/*`

### 2. Better stream cursors and reconnect logic

Target idea:
- event cursors with epoch/seq
- stale/gap detection
- timeline window fetch
- projected vs canonical rendering

Paseo files to study:
- `packages/server/src/server/agent/timeline-projection.ts`
- `packages/server/src/server/agent/agent-manager.ts`
- `packages/app/src/contexts/session-timeline-seq-gate.ts`
- `packages/app/src/contexts/session-context.tsx`

Likely Loopflow landing zone:
- `rust/loopflow/src/lfd/events.rs`
- websocket/session event APIs
- `swift/LoopflowCore/Services/LocalEventService.swift`

### 3. Provider capability + mode abstraction

Target idea:
- model/provider metadata becomes explicit product state
- not just “stringly typed provider names”
- surface permission styles in a provider-agnostic way

Paseo files to study:
- `packages/server/src/server/agent/provider-manifest.ts`
- `packages/server/src/server/agent/provider-registry.ts`

## Study for later

### 4. Relay / remote daemon architecture

Paseo files:
- `packages/relay/src/crypto.ts`
- `packages/relay/src/encrypted-channel.ts`
- `packages/relay/src/cloudflare-adapter.ts`
- `SECURITY.md`

### 5. Worktree setup/teardown hooks

Paseo file:
- `packages/server/src/utils/worktree.ts`

Worth stealing:
- lifecycle hooks
- runtime env injection

Not worth stealing:
- managed worktree root under `~/.paseo/worktrees/...`

Loopflow’s sibling worktree convention is better for our wave model.

### 6. Shared coordination channel

Paseo file:
- `packages/server/src/server/chat/chat-service.ts`

Worth exploring:
- coordination layer for humans + agents + waves

Probably not as raw chat rooms. More likely as:
- wave notes
- coordination inbox
- structured coordination events

## Do not copy blindly

### 1. File-backed JSON persistence

Useful for speed. Not the right direction for Loopflow’s core state.

### 2. Their orchestration model as a replacement for flows/waves

Their loops + schedules + chat are good primitives.
They are not a replacement for our main idea.

### 3. Electron/mobile breadth as a strategy shortcut

Paseo’s breadth is impressive, but it comes with a lot of client-surface maintenance.
The lesson is mostly about protocol design, not “copy every surface.”

### 4. Monolithic session ownership

Do not copy the “everything ends up in session.ts” pattern.
That looks like the primary source of architectural drag in Paseo right now.

## The most interesting synthesis

Paseo is closest to the layer **under** Loopflow.

If you stack them conceptually:

- **Paseo layer**: provider runtime, session lifecycle, permissioning, streaming, reconnect, remote access, multi-client control plane
- **Loopflow layer**: work discovery, flow composition, direction, governance, PM sync, shipping

That suggests a useful product question:

> Where should Loopflow become more like Paseo at the runtime layer without giving up what makes Loopflow different at the workflow layer?

The answer from this read is:

1. **Normalize provider activity more aggressively**
2. **Strengthen event/timeline semantics for Concerto and remote clients**
3. **Learn from their relay when we build remote lfd**
4. **Add worktree lifecycle hooks**
5. **Keep our wave/flow/direction/shipping system intact**

## Concrete recommendation list

### High-value, near-term

1. Add a Loopflow `ToolCallDetail`-style canonical activity model.
2. Add epoch/seq cursoring to live event streams.
3. Design a provider capability/mode registry.
4. Add repo-local worktree setup hooks.

### Medium-term

5. Build Concerto Mobile (iOS) — separate target, shared LoopflowCore.
6. Add a structured coordination surface for humans/agents/waves.

### Avoid

7. Replacing waves with ad hoc agent orchestration primitives.
8. Replacing DB-backed daemon state with local JSON stores.
9. Collapsing more daemon logic into giant coordinator files.

## Concerto Mobile: Design Decisions

Decisions from interactive design session (2026-04-06).

### Remote connectivity

**Decision: Studio relay behind Cloudflare proxy, QR-based pairing.**

```
Phone → CF PoP (edge TLS) → CF backbone → Studio (dumb relay) → lfd
```

Studio hosts a relay endpoint that pairs WebSocket connections by `daemon_id` and forwards bytes transparently. Cloudflare's existing proxy (already used for the website) gives edge TLS termination. The relay is a dumb pipe — it doesn't inspect, authenticate, or modify traffic.

**Connection model:**
- lfd opens `wss://loopflow.studio/relay/{daemon_id}` at startup, always-on, pings every 30s
- Phone opens `wss://loopflow.studio/relay/{daemon_id}` when the app launches
- Studio pairs them by daemon_id, forwards bytes transparently
- One connection per client device (phone, iPad, laptop). No multiplexing needed
- Phone speaks the exact same WebSocket protocol as local Concerto — no relay-awareness in the client

**Authentication: QR-based pairing, no Studio in the auth loop.**

lfd already mints connection tokens (64-char hex, SHA256-hashed in local SQLite ledger). The existing token system handles everything:

1. User runs `lf ops relay pair` or clicks "Pair Device" in Concerto menu bar
2. lfd mints a connection token, encodes a QR code:
   ```json
   { "relay": "wss://loopflow.studio/relay", "daemon_id": "abc", "token": "a4f8..." }
   ```
3. Phone scans QR → extracts relay URL, daemon_id, token → stores in Keychain
4. Phone connects to relay → sends `Bearer {token}` as first message
5. Studio forwards the Bearer token to lfd (transparent proxy)
6. lfd validates from its local ledger — no Studio API call, constant-time comparison
7. lfd's existing 60-second WebSocket re-validation applies. `lf ops token revoke` kills access immediately

**Why QR over Studio-distributed tokens:** Fewer moving parts. Studio doesn't need to be in the token distribution loop — lfd mints tokens and validates them locally. Studio is just a relay. QR pairing requires physical proximity once (or sharing a link), which is a natural trust anchor.

**Why not Cloudflare Durable Objects:** New platform to learn and operate. Edge latency advantage over "CF proxy in front of Studio" is marginal — relay-to-daemon hop is fixed regardless.

**Why not Fly.io:** Same architecture as Studio relay but separate deployment. Only worth it if Studio proves unstable under WebSocket load.

**Tailscale as zero-effort alternative:** Ship `lfd --bind` for power users who already have a mesh VPN. Zero code, just docs.

**Escape hatch:** If Studio-as-relay causes stability problems, move relay to Fly Machine or CF Durable Object. Client just points at a different relay URL in the QR code — no protocol change. Token validation stays local to lfd regardless.

**Effort:** ~800 lines Rust (daemon relay client + QR generation) + ~400 lines server-side (relay endpoint on Studio) + ~200 lines Swift (QR scanner + relay connection mode in LoopflowCore).

### Design constraint: the full driving spectrum

Loop mode is not passive monitoring. Users actively drive looping waves — running design reviews, reshaping implementations, making judgment calls. The automation is connective tissue between human decisions, not a replacement for them.

The mobile app must work across the full spectrum:
- **Monitoring:** Glance at wave status, put the phone down
- **Triage:** Burn through accumulated attention items quickly
- **Driving:** Read step output, send feedback, watch the wave continue

Session view and attention/triage are first-class, not afterthoughts. "Dashboard with a notification button" is the wrong mental model.

A broader question exists about differentiating these modes across all surfaces (terminal, desktop, mobile) — that's a separate design effort. For this branch, the constraint is: mobile must not be designed only for the passive end of the spectrum.

### App architecture

**Decision: Separate iOS target, shared LoopflowCore.**

LoopflowCore (13,168 lines) already targets iOS 18+. All models, networking, state management, auth, and 11 cross-platform views are shareable. Build `ConcertoMobile` as a new target with phone-optimized layouts.

```
LoopflowCore (shared)     ← 13,168 lines, already iOS-ready
├── Concerto (macOS)      ← 79 files, untouched
└── ConcertoMobile (iOS)  ← new, ~6,000-8,000 lines
```

### Provider activity normalization

**Decision: Normalize in lfd, real-time, persist to database.**

lfd parses agent output as it streams and emits typed activity events (shell, read, edit, write, search, etc.) alongside raw output. Normalized events are persisted to the database. Raw log files remain unnormalized as-is.

Schema starts small and evolves — no need to match Paseo's 10 variants on day one. The important thing is the pipeline: agent output → lfd parser → typed event → database + broadcast to clients.

Concerto (desktop and mobile) consumes typed events for structured rendering. "Agent edited src/api/routes.rs" renders differently than "agent ran cargo test."

### Timeline/stream cursoring

**Decision: Start simple, iterate.**

Add sequence numbers to WebSocket events. Implement reconnect-with-cursor. Test under real mobile conditions and evolve the model based on what breaks. Don't over-design the epoch/gap/compaction model upfront.

### Push notifications

**Decision: Skip for now.**

Focus on the relay + app experience first. Push notifications add APNs infrastructure, Studio-as-push-dispatcher, device token registration — significant complexity. The app can poll on foreground and rely on badge counts. Revisit push when the app is in users' hands and we know how they actually use it.

### Mobile token lifecycle

**Decision: Auto-refresh through the relay.**

QR pairing mints an initial token. The phone refreshes it automatically through the relay before expiry. lfd mints a new token, invalidates the old one, sends the new one back through the relay connection. The phone stores the refreshed token in Keychain.

This keeps the security property (tokens expire, revocation works) without forcing the user to re-scan a QR code every hour. The refresh interval can start generous (24h TTL, refresh at 50%) and tighten later.

Token refresh is a lightweight protocol message on the existing relay WebSocket — not a separate HTTP call. lfd validates the current token, mints a replacement, responds inline.

### Wave breakdown

**lfd wave** (new items):
- Activity normalization — typed events, DB persistence, broadcast
- Stream cursoring — sequence numbers on WS events, reconnect-with-cursor

**ios wave** (new items, sequential):
1. Studio relay endpoint (~400 lines, Studio codebase) — dumb WebSocket forwarder, pairs by daemon_id. Tracked as dependency, work happens in Studio repo
2. lfd relay client (~800 lines Rust) — outbound WS to Studio, QR generation, token refresh protocol
3. ConcertoMobile app (~6,000–8,000 lines Swift) — iOS target, QR scanner, wave list, wave detail, attention queue, session view, settings

lfd items can run in parallel with ios stage 1–2. Stage 3 (the app) benefits from activity normalization landing first but doesn't strictly block on it — the app can render raw output and upgrade to typed events later.

### Terminal on mobile

**Decision: Rendered text stream first, xterm.js WebView as follow-up.**

Ship with styled `OutputLine` rendering in a ScrollView (reuse `LiveOutput.swift`). ANSI color parsing, no cursor, no input. Covers the monitoring use case — watching agents work from your phone.

Full terminal (xterm.js in WKWebView) is a follow-up if users ask for interactive terminal access. Requires binary terminal mux protocol in lfd (~300 lines Rust).

### Attention & permissions

**Decision: APNs push with inline actions.**

Push notification when attention item is created. Action buttons on the lock screen (approve/deny/skip). Studio acts as the push relay — lfd notifies Studio, Studio dispatches via APNs.

Polling with badge count as fallback for users who deny push permissions.

## Appendix: Code-Level Detail

### A. Agent Manager internals (`agent-manager.ts`, ~2500 lines)

**State machine enforcement.** The `ManagedAgent` discriminated union makes `session: null` on `ManagedAgentClosed` — TypeScript rejects code that accesses a session on a closed agent. Good type discipline.

**Cancel flow.** `cancelAgentRun` calls `session.interrupt()`, waits up to 2s for the turn to settle via `Promise.race`. If still stuck, force-dispatches synthetic `turn_canceled`. Comment: `"foreground turn still active after timeout, force-canceling"`. Every cancel has a 2s worst-case stall.

**Run replacement.** `replaceAgentRun` sets `pendingReplacement = true` before canceling to suppress intermediate `idle` emission — prevents UI flash. But this flag is *not* reflected in the `ManagedAgent` type union, a gap in the type discipline.

**Timeline cap.** `maxTimelineItems = 200`, trims from front. No disk-based scrollback for long-running agents.

**Debug residue.** "Bug #1 Fix" / "Bug #3 Fix" comments in `waitForAgentEvent` are production debug residue. Empty catch blocks on `refreshRuntimeInfo`/`refreshSessionState` silently swallow provider garbage.

### B. Provider implementation details

**Claude (~2800 lines).** `TimelineAssembler` is a streaming diff engine — tracks `emittedAssistantLength` to emit only new character ranges. Synthetic message IDs (`synthetic-message-${++counter}`) when SDK lacks stable IDs create potential discontinuity between history replay and live stream. `spawnClaudeCodeProcess` overrides Node/Bun path resolution — documented workaround for bundled runtime mismatch.

**Codex (~2900 lines).** Full JSON-RPC client over stdin/stdout — ~250 lines of transport code that would vanish if Codex exposed a stable SDK. 90-second turn start timeout for cold-start model loading. Plan mode routes through a completely different code path (`collaborationMode/list` + fuzzy name matching) invisible to the generic `AgentSession` interface. ~30 distinct notification types parsed through ~30 Zod schemas, many existing because Codex renamed fields across versions (`call_id` → `callId`, `aggregated_output` → `aggregatedOutput`).

**OpenCode (~1200 lines).** Process-global singleton `OpenCodeServerManager.getInstance()` with settings-key mismatch warning that doesn't bubble to the user. Zod parsing handles three schemas for tool call ID field alone — fragile normalization that will silently drop calls if OpenCode changes shape. `listPersistedAgents` returns `[]` with `// TODO`.

**ACP (~1500 lines).** Most capable provider — bidirectional requests (agent calls back for terminal I/O, file reads/writes). Base class for Copilot and Pi adapters. NDJSON stream transport.

**Total provider code: ~7000 lines.** A meaningful fraction is defensive parsing for schema variants that shouldn't exist if the underlying protocols were stable.

### C. session.ts field inventory (the god object)

```typescript
// Simultaneously manages:
private readonly ttsManager: TTSManager;
private readonly sttManager: STTManager;
private readonly dictationStreamManager: DictationStreamManager;
private voiceTurnController: VoiceTurnController | null;
private agentManager: AgentManager;
private readonly agentStorage: AgentStorage;
private readonly projectRegistry: ProjectRegistry;
private readonly workspaceRegistry: WorkspaceRegistry;
private readonly chatService: FileBackedChatService;
private readonly scheduleService: ScheduleService;
private readonly loopService: LoopService;
private readonly checkoutDiffManager: CheckoutDiffManager;
private readonly activeTerminalStreams: Map<number, ActiveTerminalStream>;
private readonly workspaceGitWatchTargets: Map<string, WorkspaceGitWatchTarget>;
// ... ~20 more fields
```

Constructor takes ~25 fields, 13 of which are services. It's agent proxy + terminal mux + git watcher + voice manager + dictation pipeline + push notification router + file explorer + git operator + worktree manager + chat/loop/schedule relay. All in one file. Four companion test files.

### D. WebSocket binary multiplexing detail

```typescript
// Frame: [opcode 1B] [slot 1B] [payload...]
TerminalStreamOpcode = { Output: 0x01, Input: 0x02, Resize: 0x03, Snapshot: 0x04 }
```

256 terminal slots. Binary frames distinguished from JSON because opcodes 0x01–0x04 can't be the first byte of a JSON object. Flow control watermarks: 256KB high, 16KB low — terminal output paused when WS buffer fills. Multi-socket per session via `sockets: Set<WebSocketLike>` — designed for relay + direct dual attachment.

### E. Loop service limitations

Static worker prompt every iteration — no mechanism to vary based on failure reason. Verification is two-tier (shell check → verifier agent with structured JSON `{ passed, reason }`), retries up to 2 times on invalid JSON. On daemon restart, all running loops become `"stopped"` — no resume from mid-iteration.

### F. Schedule service: `--target self` pattern

The `target: { type: "agent", agentId }` path calls `ensureAgentLoaded` → checks live memory → falls back to `resumeAgentFromPersistence`. This enables the orchestrator pattern: an agent schedules its own wake-up. Missed ticks while daemon is offline fire once on startup, not N times.

### G. Chat mention injection

When `@agent-id` is posted, `notifyChatMentions` injects a notification string into the mentioned agent's conversation as a user-turn message via `sendAgentMessage`. The agent receives it mid-run as if a human typed it. `@everyone` expands to all non-archived, non-internal agents.

### H. Committee skill pattern

Two high-reasoning agents (`claude/opus --thinking on` + `codex/gpt-5.4 --thinking medium`) in parallel, same prompt, analysis-only (`NO_EDITS` suffix required). After convergence, separate implementer launched. Committee stays alive through review to catch drift. After ~10 iterations without convergence, start fresh. No loopflow equivalent — we could express this as an And step with two agents + Xor routing.

### I. Testing gaps

338 test files total. Only `packages/server` runs tests in CI on PRs. `packages/app` (120 test files) and `packages/cli` (37 test files) have zero CI. Biome linter explicitly disabled (`"linter": { "enabled": false }`). Format checking not in CI. Server vitest forces `singleFork: true` — serialized test suite, likely because tests share daemon port state.

### J. Model listing approaches

- **Claude**: static list of 4 hardcoded models with regex normalizer. Goes stale when Anthropic releases new models
- **Codex**: live via `thread/listModels` on app-server subprocess + `normalizeCodexModelLabel()` for casing
- **OpenCode**: fully live via `client.provider.list()` with capability metadata (reasoning, attachments, tool_call, cost). Richest and most accurate

### K. CLI design details

Docker-style command surface. Top-level shortcuts (`ls`, `run`, `attach`, `logs`, `stop`, `send`, `inspect`) for the happy path; nested subcommands (`agent`, `daemon`, `worktree`, `permit`, `schedule`) for advanced ops.

Every command returns typed `SingleResult<T>` or `ListResult<T>` with an `OutputSchema` declaring column widths, field mappings, and color functions. A single `render()` call handles table/JSON/YAML/quiet. The `--output-schema` flag validates agent output against a JSON schema with auto-retry (up to 2 attempts) — clever for scripting.

The CLI has no special privileged access — same WebSocket protocol as the mobile app. `--host` allows pointing at any daemon.

Rough edge: `paseo logs` outputs directly to stdout without the structured output layer — `--json` does nothing useful on it.

**Comparison to loopflow:** `lf` is workflow-oriented (`lf design`, `lf implement`, `lf gate` — semantic steps). Paseo's CLI is operational (`paseo run`, `paseo stop`, `paseo logs` — Docker/kubectl-style process management). Different philosophies: loopflow says "do the next thing in the workflow," Paseo says "manage your agents." Their output formatting layer is more polished than ours.

### L. App architecture

Expo app with Zustand state management. Central `useSessionStore` keyed by `serverId`. Agent activity timestamps split to a top-level slice (`agentLastActivity`) to prevent cascade re-renders — thoughtful optimization.

Binary tree layout for split panes (`SplitGroup` with `left`/`right` children, `SplitPane` leaves, max depth 4). Layout actions are pure functions tested separately from the store.

Terminal rendering: xterm.js with WebGL renderer, image support, ligatures, clipboard, search, web links. Operation queue serializes writes/clears/snapshots. Exposed on `window.__paseoTerminal` for debugging.

Connection model (`host-runtime.ts`): tracks multiple named connections per host (direct TCP, Unix socket, pipe, relay), probes in parallel, selects best available. Distinguishes `initial_loading` vs `error_after_ready` for better UX on transient failures.

### M. Desktop app (Electron)

Daemon runs as detached child (`stdio: ['ignore', 'ignore', 'ignore']`) — survives Electron restarts. Polls via `paseo daemon status --json`. Version mismatch detection restarts daemon automatically on app update.

IPC via single `paseo:invoke` channel with command registry. Local transport uses Unix sockets or named pipes (no TCP). `PASEO_DESKTOP_MANAGED=1` env var distinguishes managed vs standalone mode. CLI passthrough (`paseo open /path`) routes to running instance via `ipcMain.handle("paseo:get-pending-open-project")`.

### N. Error handling and resilience

**Daemon crashes**: Desktop polls `process.kill(pid, 0)`. SIGTERM → 15s → SIGKILL → 3s. Startup: 1.2s grace, then 200ms polls up to 30s.

**Agent crashes**: Timeline survives (append-only, file-backed). Session resume via provider-level persistence handle.

**Lost connections**: App distinguishes `error_after_ready` vs `error_before_first_success`. `queuedMessages` map holds sends during disconnection for delivery on reconnect.

**Stale state**: Epoch-based sequence numbers. Cursor from old epoch flagged `staleCursor: true` → full reload. `gap: true` in fetch → reload.

### O. Nix infrastructure

Strongest piece of their build story. Multi-platform packages (x86_64-linux, aarch64-linux, x86_64-darwin, aarch64-darwin). NixOS module (`services.paseo`) for self-hosting. Dev shell with `nodejs_22` + `python3`. Source filtered to exclude app/website/desktop (daemon packages only). Handles `node-pty` native rebuild while skipping speech modules that degrade gracefully. Custom `fix-lockfile.mjs` workaround for npm lockfile bug, verified in CI.

## LOC comparison

Nearly identical total size. Remarkably different feature surface.

| | Paseo | Loopflow |
|---|---|---|
| **Source** | 126,338 (TypeScript) | 124,292 (91.6K Rust + 1.9K Python + 30.8K Swift) |
| **Tests** | 71,923 | 74,535 (65.4K Rust + 1.7K Python + 7.4K Swift) |
| **Total** | **198,261** | **198,827** |

### What each codebase buys with ~125K source lines

**Paseo:**
- Multi-provider runtime (4 providers, ~7K lines in provider adapters alone)
- Cross-platform clients (app 39.5K, CLI 10.4K, desktop 2.9K, relay 1.2K)
- WebSocket protocol + binary terminal mux
- Chat, loop, schedule services
- Voice/dictation stack

**Loopflow:**
- Wave orchestration engine (flows, DAGs, Xor, triggers, crons)
- Two database backends (SQLite + Postgres) with migrations
- Full git/PR lifecycle (land, combine, release, rebase)
- PM integration (Notion, Linear, Asana)
- Prompt/context assembly system
- Native macOS app (Concerto, 30.8K Swift)
- Provider auth system

### Largest files

| Paseo | Lines | Loopflow | Lines |
|---|---|---|---|
| `session.ts` | **8,202** | `prompt.rs` | 3,723 |
| `codex-app-server-agent.ts` | 4,013 | `provider_auth.rs` | 3,498 |
| `claude-agent.ts` | 3,914 | `store/mod.rs` | 3,196 |
| `daemon-client.ts` | 3,813 | `flow.rs` | 2,540 |
| `messages.ts` | 2,853 | `waves.rs` | 2,284 |

Paseo's top file is 2.2x larger than loopflow's. Top 5 Paseo files: 22,795 lines. Top 5 loopflow files: 15,241 lines. Paseo concentrates more logic in fewer files.

### Efficiency verdict

**Loopflow gets more done per line.** Same total budget delivers: two database backends, three languages, a native desktop app, PM integrations, a complete shipping pipeline, and the entire wave/flow/direction abstraction. Paseo spends a large chunk on provider normalization (~7K lines), the session god object (8.2K lines), and client surfaces (app + desktop = 42K lines).

Part of this is language density — Rust is more compact than TypeScript for systems code. Part is architectural focus — loopflow doesn't spend lines on a mobile app or voice stack. But the core signal: **loopflow's largest file is 3.7K lines; Paseo's is 8.2K.** Complexity is distributed more evenly.

### By package (Paseo source only)

| Package | Lines | % |
|---|---|---|
| server | 71,277 | 56% |
| app | 39,516 | 31% |
| cli | 10,385 | 8% |
| desktop | 2,851 | 2% |
| relay | 1,190 | 1% |
| website | 696 | <1% |
| highlight | 266 | <1% |
| expo-two-way-audio | 157 | <1% |

Over half the codebase is the daemon. The app is the second-largest surface — 39.5K lines of React Native/Web.

## Final take

Paseo feels like a strong, fast-moving **agent control plane** with real implementation depth in:
- provider normalization
- live streaming correctness
- remote access
- multi-surface clients

Loopflow feels like a stronger **software-delivery operating model** with real implementation depth in:
- workflow composition
- prompt/context assembly
- wave identity and governance
- PR/merge-queue shipping
- PM integration
- durable daemon storage

The right move is not imitation.
It is selective theft.

Steal their runtime sharpness.
Keep our orchestration thesis.

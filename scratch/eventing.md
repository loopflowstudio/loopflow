# Live telemetry for the wave model

Status: design proposal. No code. Supersedes the `/ws` + `/events` split.

---

## 1. The model, in one picture

There is no telemetry center. There never was a reason for one — the machine
already has a durable registry (the SQLite ledger, `lfdb`) that every `lf` and
every wave server writes to directly. Discovery and history are **queries**
against that registry. Live motion is a **per-wave SSE stream** off the wave
server that owns the motion. Nothing aggregates; nothing tails another
process's files.

```
                        ┌────────────────────────────────────────────┐
   DISCOVERY / HISTORY  │  lfdb (SQLite)  —  the machine registry      │
   (snapshot, pull)     │  waves · runs · run_events · sessions ·      │
                        │  attention · provider_auth                   │
                        └───────────────▲───────────────▲──────────────┘
                             writes │           │ reads (lf query)
                        ┌───────────┴──┐   ┌─────┴───────────────────┐
                        │ wave server  │   │  lf  (daemonless hands  │
                        │  (per wave)  │   │   + query surface)      │
                        └──────┬───────┘   └─────────────────────────┘
   LIVE MOTION (push, SSE)     │  GET /events   (one stream per wave)
   one connection per wave     │  frames: state · turn · memory · op · inbox
                        ┌──────▼────────────────────────────┐
                        │  client (Concerto / lf sub)        │
                        │  snapshot from lf, then subscribe   │
                        │  one SSE per wave it is watching     │
                        └────────────────────────────────────┘
```

Two altitudes, two transports, cleanly separated by **durability**:

| Concern | Where it lives | Shape | Why |
|---|---|---|---|
| Which waves exist (incl. stopped) | `lfdb`, via `lf` query | pull, snapshot | a stopped wave emits nothing; only the registry knows it |
| Run / flow / step **history** | `lfdb.run_events`, via `lf` query | pull, snapshot | already durably written by `lf`; the past is not a stream |
| Session list, attention, auth **state** | `lfdb`, via `lf` query | pull, snapshot | machine facts, not one wave's motion |
| A wave's live conversation | wave `/events` SSE | push, live | the turn is happening now, in that process |
| A wave's live run/flow/step **motion** | wave `/events` SSE (`op` frame) | push, live | the wave already observes its own workers |
| Live auth / attention **transitions** | see §5 | push, live | rare, machine-wide — the one honest gap |

The rule that sorts every event: **if it is durable, you query it; if it is
motion, you subscribe to the process making the motion.** The old `/ws` broke
this rule by streaming durable facts (run/flow/step, sessions) that were
*already in the ledger* — it was a live view of a database, which is just a
query wearing a socket.

---

## 2. Why the center has to go

`lfd`'s `/ws` (see `rust/loopflow/src/lfd/http/routes/ws.rs`) is one WebSocket
that streams **everything on the box**: a snapshot from `list_waves(None)`, a
broadcast bus for wave lifecycle + auth + attention + sessions, and a per-run
**journal-file tailer** that reconstructs run/flow/step events
(`lfd/types/event.rs`, `From<LfEvent>`). Concerto's dashboard holds one socket
and sees the whole machine.

Three things are wrong with it under the wave model:

1. **It re-streams the ledger.** `run_events` is a SQLite table written
   *directly by `lf`* (`lfdb/mod.rs`: "written directly by `lf` (and by
   `lfd`)"). The `/ws` tailer reads per-run journal *files* and rebuilds the
   same events that already sit, durably and queryable, in the store. Two
   sources of truth for one fact.

2. **It is a center in a model that abolished centers.** The wave server is
   sovereign and reactive; `lf` is daemonless. `/ws` reintroduces exactly the
   always-on aggregator the redesign dissolves. Every wave's motion has to
   *reach* the center (broadcast bus, output hub, file tailer) so the center
   can re-emit it. That plumbing is the daemon.

3. **The WebSocket buys nothing.** `/ws` is push-only — the client's inbound
   half is used only for pong and a malformed-message counter. A duplex
   transport for a simplex problem. SSE carries the identical payload with
   reconnection (`Last-Event-ID`) for free.

The direction is ratified: the aggregate is a **performance proxy** for the
future OAuth/company deployment (§6), not part of the conceptual core. Get the
core right with per-wave SSE + `lf` queries; add multiplexing later without
touching the contract.

---

## 3. What opencode and codex actually do (and what to steal)

Both are single-agent servers, not fleet managers — but both solved *exactly*
the snapshot-then-live and discovery problems we have, and neither built a
streaming center. The shared skeleton is the thing to steal.

### opencode — one bus, many sessions, `{entity}.{action}`

opencode runs one HTTP server (Hono/Bun) per project directory. Our own
harness already speaks it (`harness/opencode.rs`, `harness/opencode_mapping.rs`):

- **Discovery is a list endpoint, not the stream.** `GET /session` returns
  `Session[]`. The client learns what exists by *asking*, then subscribes.
- **One SSE stream carries every session, tagged.** `GET /event` opens with a
  `server.connected` frame, then streams bus events. Each event's payload
  carries the `sessionID`; the client fans out by filtering. Our mapping does
  precisely this — `ReaderState::accepts()` drops any frame whose
  `properties.sessionID` isn't the one it wants (`opencode_mapping.rs:29`).
- **Events are a flat, typed, fire-and-forget bus.** `BusEvent.define()`,
  `Bus.publish()` after each state mutation, names like `session.status`,
  `message.part.updated`, `session.diff`, `session.error`. The publisher never
  waits for a subscriber. Multiple clients subscribe to the same stream and
  converge on the same reactive state.

**Steal:** the list-endpoint-for-discovery + single-tagged-stream-for-motion
split; the `{entity}.{action}` naming; fire-and-forget.

**Reject:** one stream for *everything*. opencode's "everything" is one
project's handful of sessions on one server. Our "everything" is a whole
machine of independent wave processes — funnelling them into one stream *is*
the center. We keep the tagged-stream idea but scope each stream to a wave.

### codex — JSON-RPC app-server, discovery is a list RPC over disk

codex's app-server (`harness/codex.rs`, targeting 0.142.5) is a single
persistent process serving many threads over JSON-RPC-lite as JSONL on stdio:

- **Thread / Turn / Item** — a Thread is the durable session (persisted as a
  JSONL rollout log on disk), a Turn is one user request's worth of work, an
  Item is the atomic unit with a `started → delta* → completed` lifecycle.
  This is *the same shape* as our Turn/ConversationItem — our
  `codex_mapping` already collapses `item/started`, `item/*/delta`,
  `item/completed` into `ConversationEvent`s.
- **Discovery is `thread/list`, an RPC over the durable logs** — cursor
  pagination plus `cwd`, `archived`, `searchTerm` filters. Not filesystem
  scanning by the client; not the notification stream. `thread/loaded/list`
  separately reports what's *in memory* (i.e. "running"). The durable list and
  the live list are **two different questions with two different calls.**
- **Resumption:** `thread/resume` by id reopens a durable thread so new turns
  append. History replays from the log.
- **Live motion is turn-scoped notifications** — `turn/started`,
  `item/started`, `item/**/delta`, `item/completed`, `turn/completed{status}`
  — emitted only while a turn runs. Between turns, silence; the log is the
  truth.
- **Server-initiated requests** ride the same channel (approval prompts pause
  the turn). Bidirectional, but only for the one thing that needs it.

**Steal:** discovery-is-a-list-RPC-over-the-durable-store; the explicit split
between *durable list* (`thread/list`) and *loaded/running list*
(`thread/loaded/list`); notifications are live-only, the log is durable truth.

**Reject:** stdio/JSON-RPC transport (it's a single-parent-process protocol;
our waves are independently reachable HTTP servers) and the in-process thread
registry (our registry is the shared SQLite ledger, reachable with no server
running at all).

### The pattern both converge on

```
   DISCOVERY   =  a query over the durable store   (GET /session ; thread/list)
   RUNNING-NOW =  a second, cheaper query          (thread/loaded/list)
   MOTION      =  live-only deltas, one stream, tagged, fire-and-forget
   HISTORY     =  replay from the durable log, not from the live stream
```

Neither tool has a machine-level streaming hub. codex is one process but
discovery is a *list call over disk logs*, not a firehose. opencode multiplexes
one project's sessions, not a fleet. **loopflow's registry (`lfdb`) is a
better durable store than either has** — it already spans every wave, running
or not, on the box. We are closer to done than the `/ws` code suggests; we
just have to stop streaming what the ledger already holds.

---

## 4. Discovery: snapshot from `lf`, no streaming center

A stopped wave emits no stream, so discovery **cannot** be a subscription — it
must be a query. The registry already has the answer:
`SqliteStore::list_waves(None)` returns every wave regardless of whether a
server is up (`registry.rs`: "the db IS the registry"). That is exactly the
snapshot `/ws` builds on connect — but it's a *query result*, not a socket
event, and it belongs to `lf`, not a daemon.

**Discovery surface (all `lf` queries over `lfdb`, no server required):**

```
lf ls              # every wave: name, status, live-endpoint?, last activity
lf ls --json       # same, machine-readable — Concerto's dashboard snapshot
lf status <wave>   # one wave: runs, workers, attention, mind state if live
lf runs <wave>     # run/flow/step history from run_events (the ledger)
```

`lf ls` answers "which waves exist" for **running and stopped alike**: it reads
`waves` from the store, and for each cross-references the `.wave-endpoint`
pointer + `/health` probe (`wave/server.rs::live_endpoint`) to mark which are
live. A running wave has an endpoint you can subscribe to; a stopped one is a
row with no endpoint — visible, inert, restartable.

**Snapshot-then-subscribe, the whole client lifecycle:**

```
1. snapshot   →  lf ls --json                 # what exists, who's live
2. for each live wave the client cares about:
       subscribe → GET http://<endpoint>/events   # that wave's motion
3. on SSE drop  →  re-query lf ls, re-subscribe live endpoints
4. history on demand → lf runs <wave> / GET /conversation   (replay, not stream)
```

The snapshot is a point-in-time read; the subscription is live-only. A wave
that starts *after* the snapshot shows up on the next `lf ls` — Concerto
re-queries on a slow cadence (or on user focus), the same way codex's
`thread/list` is re-polled. No event says "a new wave was born" because no
process is guaranteed to be listening when it is; the registry is the
authority and you *ask* it. (§6's proxy can add a born/died push later as a
pure optimization.)

---

## 5. Where the old `/ws` payloads go

Sorting each `/ws` event type by the durability rule from §1:

### Run / flow / step lifecycle → ledger query + live `op` frame

- **History / snapshot:** `lf runs <wave>` over `run_events`. This *is* the
  data the `/ws` file-tailer was reconstructing — read it from the ledger
  where `lf` already wrote it. Delete the journal-file tailer and the
  `From<LfEvent> for Event` bridge; the file journal stays as the local
  breadcrumb, but nothing streams it.
- **Live:** the wave server already observes its own workers
  (`wave/registry.rs::StoreObserver` journals `RunObserved` / `RunCompleted`).
  It emits these as a new SSE frame on its own `/events`:

  ```
  event: op
  data: {"kind":"run.started","run_id":"…","flow":"build","step":"implement","index":0,"ts":"…"}
  ```

  `op` is the wave's **operational** channel, riding the same stream as
  `turn` / `state` / `memory`. One connection per wave now carries both the
  conversation *and* the run/flow/step motion of that wave's workers — because
  both are that wave's motion. A dashboard tile for a wave is driven entirely
  by that wave's single SSE.

  Should conversation and operational events share one stream? **Yes.** They
  share a subject (this wave), a lifetime (this server), and a consumer (the
  wave's card/pane). Splitting them would mean two connections per wave to
  reassemble one picture — the N-connections problem, doubled, for no
  isolation benefit. Keep them one stream, distinguished by SSE `event:` name
  (`turn` vs `op`), exactly as `turn`/`state`/`memory` already coexist.

### Session list → ledger query (`op` frame for live wave-scoped changes)

The machine session list is a registry read: `lf status` / a `lf sessions`
query over the `sessions` table. Concerto's agent tree is a snapshot from that,
refreshed on the `lf ls` cadence. A session that belongs to a specific wave
(worker runs, the `wave_server` row) surfaces its live transitions as that
wave's `op` frames. There is no machine-wide "a session changed" push in the
base model — the tree is a query result that re-reads, not a live-maintained
mirror.

### Attention → ledger query (+ optional wave `op` frame)

Attention items are rows (`attention` table, `AttentionItem`). Snapshot via
`lf` query; a wave-scoped attention item (a wave needs input) can ride that
wave's `op` frame so its card lights up live. Machine-wide attention with no
owning wave is a query that re-reads — rare enough that snapshot cadence is
fine, and §6's proxy can push it later.

### Auth (provider_auth) → ledger query + a machine `lf watch auth` stream

Auth is the one genuinely machine-level, genuinely *live* concern with no
owning wave: a device-code flow's `verification_uri` must reach the UI the
instant it's minted, and token-refresh failures must surface immediately.
This does **not** justify resurrecting `/ws`. Two honest options, pick per how
much the UX needs push:

- **(a) Query + short poll.** Auth state is a registry read
  (`lf auth status`); Concerto polls it on the auth screen only. Simplest;
  device-code UX tolerates a 1s poll.
- **(b) A dedicated, single-purpose auth SSE on `lf`.** `lf` gains one tiny
  `GET /auth/events` SSE (or `lf watch auth` for the CLI) that carries *only*
  the six `auth.*` events. It is not a machine aggregator — it streams one
  narrow, ownerless concern. This is the smallest possible live surface and
  keeps the "no center" property: it multiplexes nothing, tails no files,
  aggregates no waves.

Recommendation: ship **(a)** in the base model (auth screens are transient and
polling is invisible there); reserve **(b)** for when device-code UX demands
instant push. Either way, auth never rejoins wave telemetry.

### Wave lifecycle (created/started/stopped/waiting) → ledger query

These were `/ws` events carrying the enriched `WaveDto`. They become `lf ls`
snapshot + re-query. "Waiting" (a wave needs a human) is the interesting one:
while the wave is live it's a `state` frame on its own `/events` already; the
registry row also reflects it, so a re-query catches it for a client not
subscribed to that wave.

---

## 6. The proxy: performance, added later, over an unchanged core

The base model costs a client **N SSE connections** for N waves it watches,
plus periodic `lf ls` re-queries. For a laptop with a handful of waves this is
nothing. For the future company/OAuth deployment — many waves, remote clients,
one browser tab watching a fleet — N connections and N re-queries over the
public internet is the real cost. That is a **performance** problem, and it
gets a **performance** answer, bolted on *over* the conceptual model without
changing a single wave's contract.

**The aggregation proxy** is an optional process that:

1. **Fans in.** Holds the N per-wave `/events` SSE connections locally (cheap,
   same box / same LAN as the waves) and the ledger. It is a *client* of the
   exact same `/events` + `lf` surfaces defined above — it invents no new
   wave-side contract.
2. **Fans out one multiplexed SSE** to each remote client:
   `GET /fleet/events`, where every frame is a base frame **tagged with its
   `wave`**. This is opencode's move — one stream, every subject tagged — but
   applied at the fleet tier, off to the side, not baked into each wave.

   ```
   event: op         data: {"wave":"goals","kind":"run.started",…}
   event: turn       data: {"wave":"systems", … , "channel":"systems.148e"}
   event: wave       data: {"wave":"meta","kind":"born"}        # discovery push
   ```
3. **Serves discovery as push.** Because the proxy holds the ledger and every
   live stream, it *can* emit `wave.born` / `wave.died` the instant they
   happen — the one thing the base model makes you poll for. This is the
   proxy's whole reason to exist for discovery: turning the re-query into a
   push. It is an optimization, not a new fact.
4. **Terminates OAuth.** One authenticated multiplexed connection replaces N
   token-bearing wave connections. This is where the old `/ws` auth
   revalidation actually belonged.

Critically: **remove the proxy and the model still works.** Every client can
fall back to `lf ls` + N direct `/events`. The proxy is a cache/multiplexer
with no authority — it holds no truth the ledger and the waves don't already
hold. It is exactly what `/ws` *pretended* to be but wasn't: `/ws` was load-
bearing (it owned the file-tailer's reconstruction of run events); the proxy
owns nothing. That is the whole difference between a center and a cache.

Base contract the proxy must never change:

- per-wave `GET /events` frame shapes (`state`/`turn`/`memory`/`op`/`inbox`)
- `lf ls` / `lf runs` / `lf status` query outputs
- the durability rule: the proxy may *push* what the base model lets you
  *query*, but it may never be the sole source of any fact.

---

## 7. Migration: two clients collapse into one

Today Concerto runs two disjoint streaming clients (they share no transport,
schema, or store abstraction):

- **`EventService`** — `swift/LoopflowCore/Services/LocalEventService.swift`
  (`public actor EventService`, alias `LocalEventService`). WebSocket on
  `lfd /ws`, Bearer + cert-pinned, backoff reconnect. `parseEvent` handles a
  broad multiplexed set (`connected` snapshot, `wave_*`, `agent_*`,
  `output_line`, `attention_*`, `session_*`, `auth.*`). It fans into
  `WaveStore` / `AttentionStore` / `RunStore` / `authProviderStore` /
  `OutputBuffer` via `swift/LoopflowCore/State/RepoState.swift` (`subscribe`
  wiring around `:494`). This is the whole-machine dashboard feed.
- **`WaveChatConnection`** — `swift/LoopflowCore/Services/WaveChatClient.swift`
  (`@MainActor @Observable`). Per-wave SSE on `http://<endpoint>/events`
  (endpoint from `wave/<name>/.wave-endpoint`), hand-rolled `SSEFrameParser`.
  Handles `state` / `turn` / `memory`; holds `turns`, `phase`, `mindState`,
  `memorySummary`. Consumed by the chat pane
  (`swift/Concerto/Platform/macOS/Views/WaveChatView.swift`).

They collapse into **one SSE client plus a query client**:

1. **`WaveChatConnection` grows into the per-wave client.** It already parses
   `state` / `turn` / `memory` and is the Swift twin of
   `wave/subscription.rs`. It gains the `op` frame. Both the chat pane **and**
   the dashboard card for a wave read from this one connection — a card's
   run/flow/step progress is the `op` frames it subscribes to. The per-wave
   half of `EventService` (`wave_started/stopped/waiting`, `agent_*`,
   `output_line` for that wave) moves here. One SSE per watched wave.
2. **`RegistryQuery` (new, thin).** Wraps the discovery/history queries —
   `lf ls --json` / `lf status` / `lf runs`, or the equivalent `lf`-served
   HTTP (the lfd REST routes `/waves`, `/runs`, `/attention`, `/sessions`,
   `/auth` already exist in `lfd/http/mod.rs`; in the daemonless world `lf`
   serves the same reads over the shared store). It feeds `WaveStore` /
   `AttentionStore` / `RunStore` from *snapshots*, on a re-query cadence + on
   user focus — replacing everything `EventService` got from the `/ws`
   `connected` snapshot and its durable-fact events. Note `lf ls` is **new**:
   today `lf --list` is the step/flow catalog, and wave listing is not exposed
   on the CLI at all — only through lfd's DB view.
3. **`EventService` (WebSocket) is deleted.** Every live wave/run/session/
   attention event it carried is either a `RegistryQuery` re-read (durable
   facts, from the `waves` / `run_events` / `attention_items` /
   `terminal_sessions` tables) or a per-wave `op` frame (that wave's motion).
   Its `auth.*` events become the §5 auth path (poll, or the narrow `lf` auth
   SSE). `RunStore` now sources run/flow/step from `lf runs` (the ledger it
   was always in) plus live `op` frames, not from lfd's file-tailer bridge.

The dashboard's mental model flips from *"one socket pushes the whole
machine"* to *"query the registry for the shape, open a live stream per wave
I'm actually watching."* Fewer moving parts: one SSE frame parser (already
shipped, twice — Swift `SSEFrameParser` and Rust `wave/subscription.rs`), one
query client, no WebSocket, no `lfd/journal.rs` file-tailer, no
broadcast/output hubs in `lfd`.

When the fleet grows past comfort, point Concerto's one SSE client at the §6
proxy's `/fleet/events` instead of at N wave endpoints — same frame shapes,
now tagged with `wave`, one connection. The migration to remote is a change of
*URL and fan-out*, not of model.

---

## 8. Review — the hard questions

- **One screen?** Yes: durable ⇒ query `lf`/`lfdb`; motion ⇒ subscribe to the
  wave. Everything sorts by that one rule.
- **Maps to the real thing?** A wave's stream carries a wave's motion. The
  registry holds the machine's durable facts. The old `/ws` mapped to an
  implementation accident (a daemon that happened to see everything).
- **2 a.m.?** A wave stream drops → the client re-queries `lf ls` and
  reconnects that one wave; every other wave is unaffected (no shared socket
  to take down). The ledger is the fallback for every durable fact. Auth
  failures surface on their own narrow path.
- **Does the proxy earn its keep?** Only in the fleet/remote deployment, and
  only as a multiplexer/cache — it's absent and unmissed on a laptop.
- **Would deleting code make the system truer?** Yes — deleting the `/ws`
  file-tailer, the `From<LfEvent> for Event` bridge, the broadcast/output
  hubs, and Concerto's `EventService` removes a second source of truth for
  run events and an entire transport. The system gets smaller and more true.

## Open questions

- Does `lf` expose the discovery/history queries as **CLI only**, or also as a
  small `lf`-served HTTP query surface for Concerto? (Concerto shelling out to
  `lf ls --json` vs. hitting `GET /waves` on a local `lf` query port. Leaning
  HTTP for the app, CLI for humans — same query underneath.)
- `op` frame vocabulary: mirror `run_events` node/event names 1:1
  (`run.started`, `step.completed`, …) so the ledger row and the live frame
  are the same shape — a client folds both with one code path. Confirm no
  field drift between the ledger row and the frame.
- Auth: ship poll-only (5a) first, or build the narrow `lf` auth SSE (5b) up
  front because device-code UX is a launch surface?

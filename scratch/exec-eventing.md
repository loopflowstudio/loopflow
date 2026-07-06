# Exec + Eventing — buildable spec

Two ordered slices. Companion deep-dives (same PR family): `scratch/lfd-exec.md`
(the exec door) and `scratch/eventing.md` (eventing rationale + opencode/codex
research). This file is the build plan — enough to run `lf` against.

## Shared model

**"an lfd" is a small kernel, not the machine daemon.** An lfd = an axum HTTP
face + the `/exec` door (run an `lf` argv locally) + token auth. The machine
`lfd` and each wave server both *instantiate that kernel* with their own state
and routes. **A wave HAS an lfd** (mounts the kernel, wave-scoped); **it ISN'T
one** (a wave is identity + mind + journal + its lfd). Do NOT embed the machine
`HttpState` (executor/event_hub/providers/github) into a wave — it's
machine-scoped and slice 2 deletes half of it. The shared piece is
`crate::lfd::lf_exec` (state-free, already built) + a shared token-auth
convention.

**The exec path:**
`lfq exec <lf argv>` → resolve endpoint (**env `LF_WAVE_ENDPOINT` first, lfdb
second**) → `POST /v0/exec` → the target lfd (machine, or the wave's server)
runs `lf <argv>` **locally, unsandboxed** → returns `{exit_code, stdout,
stderr}`. `lfq exec` mirrors the `/exec` route; `lfq` is a thin client whose
verbs mirror lfd's.

**Discovery:** env for in-wave (the executor injects `LF_WAVE_ENDPOINT` + a
per-subagent token when spawning a subagent — a sandboxed process reads its env,
not the shared db); lfdb (`WaveAgent` row `env.LF_WAVE_ENDPOINT`) for external
callers who know the wave. **Drop the `.wave-endpoint` file** — env + lfdb
suffice; one fewer source of truth.

**Auth:** the wave server's `/exec` accepts a **per-subagent capability token**
minted at spawn (least-privilege, distinct from the resident token — the
resident token must NOT authorize a subagent call). Machine `lfd` `/exec` rides
the existing bearer token.

---

## Slice 1 — Exec + `lfq`

Base: machine `lfd` `POST /v0/exec` already shipped (#825 — the `lf_exec` engine
+ route). Remaining:

1. **Wave-server `/exec`.** Mount `.route("/v0/exec", post(exec_handler))` on
   `wave/server.rs`'s existing `Router`, reusing the state-free
   `crate::lfd::lf_exec` engine. Exec in `runtime.repo_root()` (unsandboxed =
   the outwave). This is the whole "wave has an lfd" — one route.
2. **Per-subagent token.** Extend the wave server's auth so `/exec` accepts a
   valid per-subagent capability token (in addition to, not replacing, the
   resident door for the resident routes). Mint the token where the executor
   spawns a subagent. Keep it minimal — a per-wave-boot set of accepted tokens,
   or a signed token the server can verify without new schema; flag if a store
   change is needed before going deep.
3. **Env injection.** Where the executor spawns a subagent and already sets
   `LFD_SESSION_ID`, also set `LF_WAVE_ENDPOINT` (the wave's endpoint) and the
   minted per-subagent token in the child env.
4. **`lfq` binary.** `src/bin/lfq.rs` + a `[[bin]]` in `Cargo.toml`.
   `lfq exec <argv…>`: resolve endpoint (env `LF_WAVE_ENDPOINT` → lfdb) + token
   (env), `POST {argv}` to `/v0/exec`, print stdout/stderr, propagate the exit
   code. Generic passthrough — no per-subcommand mirror.

**Tests:** wave `/exec` — no token → 401, resident-only path unaffected, a
minted per-subagent token → accepted, garbage argv → 400; `lfq exec` round-trips
an argv against a door and propagates a non-zero `lf` exit; endpoint resolves
env-first.

**Done:** a sandboxed subagent runs `lfq exec op commit -m "…"` and it commits
via its wave's lfd, unsandboxed.

---

## Slice 2 — Eventing

Principle (see `scratch/eventing.md`): **durable ⇒ query it (`lf` CLI); motion
⇒ subscribe to the process making it (per-wave SSE).**

**Backend:**
1. **Query verbs.** `lf ls --json` (all waves incl. stopped, via
   `list_waves(None)`), `lf status`, `lf runs` (from `run_events`). Pure `lf`
   CLI over lfdb — no HTTP query API.
2. **`op` SSE frame.** Add to `wave/server.rs` `/events` an `op` frame carrying
   that wave's run/flow/step motion, reusing `run_events` event names 1:1. The
   wave already observes its workers (`StoreObserver`); source the frame there.
3. **Delete the aggregate.** Remove lfd `/ws` (`routes/ws.rs`), the journal-file
   tailer (`lfd/journal.rs`), the `From<LfEvent> for Event` bridge, the
   `event_hub` + `output_hub` broadcast buses, and the machine `Event` enum
   (`lfd/types/event.rs`). Trim `HttpState` accordingly.

**Concerto (Swift):**
4. Delete `EventService` — the WebSocket `/ws` client (`LocalEventService.swift`).
5. Grow `WaveChatConnection`/`WaveChatClient` to consume the `op` frame → one
   per-wave SSE client driving both the chat pane AND the dashboard card.
6. New `RegistryQuery` feeding `WaveStore`/`AttentionStore`/`RunStore` from `lf`
   snapshots — exec `lf ls/status/runs` locally, or over SSH for a remote box.

**Tests:** `lf ls/status/runs --json` shapes; `op` emitted for a run; Concerto
builds and the migrated stores render from queries + SSE.

**Done:** Concerto's dashboard runs off `lf` queries + per-wave SSE; `/ws` and
both old streaming clients are gone; one SSE client remains.

---

## Deferred (not built here)

Aggregation — one socket over all waves, fleet `wave.born`/`wave.died` — returns
only as the future company/OAuth **proxy**: a cache over this model, removable
without loss of truth. Out of scope.

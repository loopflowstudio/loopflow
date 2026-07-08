# Wave-agent follow-ups

What the wave-agent branch deliberately did not resolve: the decision
questions for a next branch, the risks we accepted with eyes open, and the
reading list for walking the merged code. The branch's `scratch/` review
artifacts die at land; this survives. When an item is picked up, file it in
Linear (`lf op pm update`) — this doc is context, not the tracker.

## Decision questions

Each is an accept-or-cut call. The governing insight: the distributed model
obviates centralized machinery, not dispatch. What remains is deciding which
centralized organs are vestigial.

> **Reconciliation 2026-07-06** (top-down audit of the merged tree + Jack
> decisions). Status of the questions below:
> - **#2, #4, #5 — moot; the organ is already gone.** The old goal-agent launch
>   path, `build_wave_agent_command`, `InFlightDispatch`,
>   `LOOPFLOW_OPERATING_PROMPT` / "Looping Agent" double-identity, `loop_ticker`
>   + the activation queue, and `roadmap_item` plumbing were all confirmed
>   *removed* from the tree. `render_goal` ceremony is stripped. No work left.
> - **#3 — moot.** `--pool` / `Placement::Pool` / `WorkerPlacementDto::Pool`
>   are gone (`Placement` is `Stack | Fresh`). `--stack` survives, as intended.
> - **#6 — DECIDED: accept immediate kill.** No grace stage now; the design
>   doc was fixed to match. (M4-if-ever.)
> - **#7 — DECIDED: accept the bounce.** Fix is worker *flexibility* (a down
>   server is a shrug), mostly an operating-instructions adaptation. Queue-for-
>   offline stays a named follow-up, not a commitment.
> - **#9 — DECIDED: hand-authored traces + a live smoke gate.** The smoke gate
>   is the real drift catcher; skip recorded traces.
> - **#10 — mostly moot:** `LFD_DISABLE_TMUX` and `alias="loop"` are gone;
>   migration-tolerance healing (048/049/050) stays until stores converge.
> - **#8 — still live:** file-roadmap leftovers to delete (Linear is the
>   roadmap). **#1** partial as noted.
> - **New from the audit:** wave = 1 repo (strip `wave_repos`); the live M1 gap
>   is "lfd stops being a hand." Two newly-dead tables
>   (`secrets_provider_config`, `wave_pr_merge_events`) being cut. See
>   `architecture-direction.md` → Current state.

1. **Two dispatch worlds. — RESOLVED (#803).** `lf q` (`lf/commands/q.rs`)
   was retired; dispatch now flows through `--dispatch`/`--wave` on the flow
   commands (one dispatch door). The lfd HTTP worker route
   (`lfd/http/routes/waves.rs` `run_worker_handler`) remains for
   dispatch-against-remote-lfd; full convergence of the route onto the same
   core is still the architecture wave's cut.

2. **The old goal-agent launch path.** `run_wave_handler` →
   `build_wave_agent_command` (`lfd/executor/wave/mod.rs`) wraps the goal
   in `LOOPFLOW_OPERATING_PROMPT` (`engine/flow.rs` — "You are the Looping
   Agent") and launches a one-shot inline run. The reactive server
   (`lf wave` + mind) is the brain now; Concerto starts waves via
   `lf wave`, and Swift's `RepoState.runWave` has zero callers. Deleting
   takes `LOOPFLOW_OPERATING_PROMPT`, the roadmap/in-flight/metrics
   ceremony in `render_goal`, and `InFlightDispatch` with it — the mind's
   journal `fold_workers` already covers in-flight. Also kills the mind's
   double identity (its seed carries "Looping Agent", its discipline says
   "mind").

3. **`--pool` / `Placement::Pool`.** Shared worktree, shared branch,
   collide-by-design — the centralized exception inside the distributed
   command. Chat + isolated branches + stacking are supposed to replace it.
   Non-CLI users are exactly the legacy machinery (activation placement,
   the HTTP route's `WorkerPlacementDto::Pool`). `--stack` survives:
   dependency ordering between unlanded runs is real in any model.

4. **`loop_ticker` + the activation queue.** The tick → activation →
   `build_wave_agent_command` loop is the centralized brain the reactive
   server replaced, kept for lfd-only waves. It now probes ghost brains
   (`live_brain_after_probe`) instead of trusting rows, and server waves
   are born `paused=true` solely to silence it. Decide: keep for
   lfd-managed waves, or cut once `lf wave` is the only brain.

5. **`roadmap_item` plumbing.** Dead end to end — every internal producer
   passes `None`; it terminates in a debug log ("no local ingest"). Rust
   side plus Swift `LocalWaveService` sender. Contradicts
   Linear-is-the-roadmap.

6. **Interrupt has no grace window.** The design's cooperative → grace →
   kill story is aspirational everywhere: codex is cooperative with a
   deadline janitor but `stop()` goes straight to group-SIGKILL; claude
   interrupt is immediate SIGKILL of the per-turn process. Accept (and fix
   the design doc), or build the grace stage.

7. **Chat queuing for offline waves.** Publish-to-no-subscriber now drops
   silently outside any wave (correct pubsub), but a resolvable wave whose
   server is down is a hard error — "mail to a dead wave bounces". Worker
   reports racing a server restart fail loudly. Queue-for-offline-waves is
   the missing piece, or accept the bounce.

8. **File-roadmap leftovers.** `wave/architecture/{2,3,4}-*.md`,
   `wave/architecture/queue/` + `proposals/` (file-based work queue —
   ready/in-flight/done), `wave/root/backlog.md`. Exactly the mirror
   LOOPFLOW.md forbids. `wave/architecture/MEMORY.md` points at "roadmap
   item 2" — update the pointer when deleting.

9. **Conformance traces are hand-authored, not recorded.** The manifest
   says `opencode_version: "unknown"`; codex hardcodes 0.142.5 wire
   strings and unknown methods silently no-op. No steer/interrupt tests at
   driver level. Highest-risk seam in the harness layer: a vendor bump can
   drift silently. Record real traces, or add a live smoke gate.

10. **Small knobs.** `LFD_DISABLE_TMUX` (read, never set anywhere);
    `alias = "loop"` on `lf wave` (compat shim); migration-tolerance
    healing (048/049) deletable once live stores converge; the hand-rolled
    HTTP GET in `engine/wave_context.rs` (~90 lines, deliberate —
    reqwest::blocking panics inside a runtime — but worth a collapse if
    that constraint ever lifts).

## Accepted risks (know they're there)

- OpenCode interrupted-status is a timing heuristic; its retry re-POSTs a
  possibly-processed message on 5xx.
- Mutex `.expect("poisoned")` throughout reader tasks — a poisoned lock
  kills a turn silently; a turn-serialize failure emits a blank SSE frame.
- `ChatTurn.sequence` sentinel: non-`turn-<int>` ids collapse to `.max`
  and order by id string — silent if the id scheme changes.
- Migration tolerance matches sqlite/postgres error-string substrings.
- SIGTERM/SIGHUP path is asymmetric with Ctrl-C: interrupt hooks
  deregister and drop the endpoint file, but the graceful-shutdown future
  is `ctrl_c()` only (child processes die via process-group/kill_on_drop).

## Reading list

Four layers, stacked. Read 1 → 3 → 2 → 4: the core defines the vocabulary,
the lf surface is its most decision-dense client, the harness is big but
mechanical once you've seen the event types, Concerto mirrors wire shapes
you'll already know.

| Layer | Where | The question to hold |
|---|---|---|
| 1. Wave server core | `rust/loopflow/src/wave/` | Is the journal really the only truth? |
| 2. Harness | `rust/loopflow/src/harness/` (hoisted out of `lfd::conversations` in #803) | Where does vendor drift bite? |
| 3. lf surface | `lf/session.rs`, `commands/{chat,memory}.rs`, `engine/wave_context.rs` | Do the doors degrade correctly? |
| 4. Concerto | `swift/` WaveChat stack | Does the viewer ever participate? |

**Layer 1 — core.** `journal.rs` (`EventKind`, `Journal::append` — the one
writer; `fold_thread`/`fold_workers` — the pure folds; `mark_consumed` is
the queue's correctness core). `runtime.rs` (`WaveRuntime::open` + boot
janitor; the `sink_*` trio; `force_finalize_open_turn`). `mind.rs`
(`run_mind` scheduler: biased select, steer degrade, interrupt deadline,
failure cap, auto-revive ladder). `state.rs` (`can_transition` — one
screen, read first). `registry.rs` (one-brain enforcement,
`live_brain_after_probe`, force-takeover). Turn growth is one function:
`ChatTurn::absorb_item` (`chat/turns.rs`, moved in #803) — live snapshot, fold,
and adapter all call it.

**Layer 3 — lf surface.** The env contract is the crux: `LFD_SESSION_ID`
without `LFD_SESSION_INHERITED` = "this process owns the row" → adopt;
with the marker = ancestor's row → register a child (`lf/session.rs`).
`chat.rs` `resolve_target` (env → worktree → store row → endpoint file) is
shared by memory. `q.rs` `dispatch` — placement, capacity gate, the exact
env handoff. `wave_context.rs` — live → journal → empty ambient fallback.

**Layer 2 — harness.** `codex.rs` `start_inner` (spawn, process group,
handshake, reader/writer tasks) and `process_notification` (shared
production+replay dispatch). The capability matrix: codex steers and
interrupts; claude neither steers nor has a session-terminal error (one
subprocess per turn); opencode manufactures turn boundaries from
`session.status`. `lf_tag.rs` buffers tags split across chunks.

**Layer 4 — Concerto.** `WaveChatClient.swift` (hand-rolled SSE parser,
id-replace upsert, composer verb table). `WaveOrigin.swift` (worktree →
origin, the choke point every path shares). `ChatTurn.swift` (manual
codec; DTO discipline is clean — no `??` fallbacks, unknown items decode
to `.unknown`).

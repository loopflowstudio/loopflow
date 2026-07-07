# Next milestone: the wave conversion

The single scope of the next run. Runs 1–2 built the flowloop runtime and two
tiers around it; both runs deferred the same two items. This slice is *only*
those items, so there is nowhere to defer to:

1. **The wave runs as a flowloop** — its persistent-vendor-thread turn is
   replaced by a `wave-pass` child; the residency's scheduler and chat shell
   remain.
2. **`mind` → `flowloop` everywhere** — names, flags, comments, docs.

Everything this slice plugs into already exists on this branch: `flowloop/`
(Tier — including `Tier::Wave` → `"wave-pass"` + `Oracle::Never` — `run_pass`,
`FlowloopRun`), the `wave-pass` builtin flow, and the `wave_clarify` /
`wave_pursue` / `wave_mutate` skills. Nothing runs them yet.

**Non-goals:** project-tier wiring, per-event delta streaming, live steer,
prod oracle, budget accounting. Charter: `scratch/flowloop.md`.

## The conversion — keep the scheduler, replace the turn

`wave/mind.rs` today: `run_mind` (:502) drives a biased select over inbox /
harness events / heartbeat / cron / interrupt deadlines, feeding one
persistent vendor thread, adapting harness events to `ResidentDelta`s via
`EventAdapter` (:322).

After: the select survives as the wake source; a wake runs **one `wave-pass`
flow** as a bounded headless child in the wave home via `run_pass(home,
Tier::Wave, seed, …)`. Per wake:

- **Message(s)** → drain the queue into the pass seed (one pass answers
  everything queued, same as today's turn-boundary drain).
- **Heartbeat** → `heartbeat_prompt` + the `<in_flight>` fold, as today.
- **Cron** → `cron_prompt`, as today.
- The goal seed (`build_goal_seed`, :259) survives as the standing preamble;
  `<lf:wave-memory>` / `<lf:wave-chat-recent>` re-fold every pass via normal
  context assembly — continuity is the log + memory, not the vendor thread.

What dies with the persistent thread (all already degraded or vestigial):

- `start_thread` / resume plumbing — resume is already a cold start on codex
  (`mind.rs:59`).
- The steer path (`on_steer`) — codex-only today (`harness/codex.rs:552`);
  queue-and-fold-at-boundary becomes universal, which is already the doctrine.
- `EventAdapter`, `USAGE_FLUSH`, the usage-wedge machinery — no in-process
  harness events to adapt.
- Cooperative interrupt + `INTERRUPT_DEADLINE` + force-close — **interrupt =
  kill the pass child**, close the turn `Interrupted`.

What stays: the listener, supervisor, journal, wire protocol, and
`MAX_CONSECUTIVE_TURN_FAILURES` (a failed pass is a failed turn; three
consecutive fail the flowloop, supervisor revives the process).

**Coarse wire.** The driver emits `TurnOpened` on wake, streams the child's
output as content deltas (`run_pass` grows an on-output line callback — it
currently buffers via `wait_with_output`), and closes with `TurnFinished`,
posting the pass's reply text to the thread. One open design point for the
builder: *which* text is the reply — check what `lf -b` actually emits and
pick the simplest honest answer (likely the final assistant text of the
`wave_mutate` phase). Acceptance is behavioral: a human-readable reply lands
in the thread.

**Heartbeat coarsens.** A pass is three harness invocations; 300s idle is a
persistent-session artifact. Default `HEARTBEAT_IDLE` → 4h for pass-based
waves (executive call — adjust in review, not in code archaeology).

**No dual mode.** Live waves flip with this PR. One implementation.

## The rename sweep

The noun is flowloop; "mind" survives only in git history. Blast radius,
measured:

- **Rust (12 files):** `wave/{mind,mod,resident,runtime,server,state,
  supervisor,wire,journal,channel}.rs`, `lf/mod.rs`, `bin/lf.rs`.
  - `wave/mind.rs` → `flowloop/wave.rs` (the driver joins its runtime;
    residency machinery stays under `wave/`)
  - `run_mind` → `run_flowloop`, `MindEnd` → `FlowloopEnd`,
    `MindConfig` → `FlowloopConfig`, `mind_agent_config` →
    `flowloop_agent_config`, etc.
  - CLI flags `--no-mind` / `--mind-only` → `--no-flowloop` /
    `--flowloop-only`, no aliases (internal surface)
  - comment/doc sweep: `wave/`, `lfd/`, READMEs, `docs/lf.md`
- **`MindState` (wire DTO):** mirrored in `wave/wire.rs`, 3 Swift files
  (`WaveChatClient.swift`, `ContractTests.swift`,
  `WaveChatConnectionTests.swift`), fixture
  `tests/fixtures/dto/wave_mind_states.json`. Rename to `FlowloopState` as
  the **last commit of the slice**; it may split into an immediate follow-up
  PR if it crowds this one — that is the only permitted deferral.

## Done when

- **Demo:** a live wave running on passes — send it a chat message, watch a
  wave-pass fire (three phases in the run log), see the reply land in the
  thread; a heartbeat and a cron each open a pass; interrupt kills the child
  and the turn closes `Interrupted`.
- `grep -riE '\bmind\b' rust/loopflow/src` returns nothing meaningful
  (`MindState` excepted iff its slice split off).
- Wave driver unit tests: seed composition per wake kind (message / heartbeat
  / cron), consecutive-failure ladder, interrupt-kills-child.
- Existing wave/listener/supervisor tests renamed and green; `lf task` tests
  untouched and green.
- DTO fixture round-trip green in all three languages if `MindState` renamed.

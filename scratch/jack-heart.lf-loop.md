# lf wave — the wave runtime, loopflow owns the outer loop

> Renamed from `lf loop`. The command doesn't name a mechanism ("a loop"); it
> names the noun it runs — the **Wave**. `lf wave <name>` = run this wave's
> standing system. "loopflow owns the outer loop" is the *doctrine*, not the
> command name. The shipped `loop.rs` is the progress arm of this, at its crudest.

## What to build

`lf wave <name>`: a **foreground, non-terminating** process that runs a wave's
whole standing system. Not a one-shot, not a detached daemon — you launch it in
your terminal and leave it running (like a dev server); Ctrl-C ends the wave. It
is the **one place** loopflow introduces a custom harness: it owns the *outer*
loop deterministically, and every agent pass it spawns is a bounded vendor-harness
run.

`lf wave` is the **runtime** — it hosts everything the wave needs (progress
loop, cron scheduler, chat API) in-process. There is **no separate `lfd` daemon
underneath**: in the long run lfd dissolves into parts of `lf`, and today's
`lfd/*` modules are **absorb-targets** that migrate *into* this runtime, not an
API `lf wave` calls across a process boundary. "Detached" is just `lf wave`
running with no terminal attached — the same code, for when you want the wave
alive with your laptop closed / reachable from a phone.

**`lf goal` / `/goal` stays, untouched** — people use it and `lf wave` must not
get in the way. `lf wave` is *additive*: its progress and cron arms **launch
`lf goal --once` tmux passes** as their inner unit. So `/goal` is both the human's
interactive command AND the bounded async unit the wave dispatches.

Today the *model* owns the loop — `/goal` interactive seeds a session and the
model is *instructed* to keep going (`LOOPFLOW_OPERATING_PROMPT`) until metrics
say done. That loop gets stuck: declares victory early, spins, loses the thread.
`lf wave` reclaims the *outer* loop for loopflow and runs bounded passes underneath.

> "we just do it in our language" — the loop logic is loopflow's, not rented
> from the vendor's agent loop.
> "each inner loop does `lf goal -b` or whatever"
> "I dont want to get in the way of people using /goal ... we use /goal for an
> async process"
> "in the long run i dont think we have lfd anymore, its all just been moved to
> parts of lf"

## Architecture: one pass-launcher + one chat API, sharing MEMORY

A Wave is **not one loop**. It's concurrent arms at different cadences, all thin
conductors coordinating through **MEMORY** (the shared brain) — they never call
each other, they only share `wave/<name>/MEMORY.md`. No single arm holds a long
session; the only long-lived thing is the deterministic Rust supervisor, which
holds **no agent context**. The only continuity is MEMORY.

**Four arms, three shapes.** (1) **Pass launcher** — progress + crons are the
*same mechanism*, a loopflow-owned outer loop whose inner unit is a bounded worker
pass; they differ only in trigger policy (repeat-on-finish vs scheduled). (2)
**Monitor** — reads worker streams, judges what's relevant, feeds chat. (3)
**Chat** — the one arm that must return a reply to a caller, so it's an
**in-process API**, not fire-and-forget. Each worker carries a tmux handle (human
attach) *and* a tee'd out/err stream (monitor input) — the two are independent.

```
lf wave <name>   (foreground, non-terminating, concurrent; hosts the whole runtime)

  PASS LAUNCHER → bounded worker passes   (loopflow owns the outer loop):
    ├─ progress trigger:  repeat-on-finish — fire the next pass the instant the
    │                     last one exits; no timer, no delay.
    └─ cron triggers:     scheduled — fire a maintenance pass on a timer /
                          cron expr (orient-daily · scan-changes · rebase · …).
    each worker gets TWO handles:
      · tmux handle    → for a HUMAN to attach + watch/steer live
      · out/err stream → tee'd to wave/<name>/streams/<run-id>.log for the monitor

  MONITOR LOOP (observer + summarizer/judge, its own tick):
    read each worker's captured out/err stream → run a summarizer/judge that
    distills what's RELEVANT → forward that into chat (+ a standing summary).
    Never parses tmux; only reads the clean batch-mode stream logs. Workers
    report nothing — no beacons; the monitor just has the streams.

  CHAT API (in-process HTTP/WS + mailbox):
    on human message: skip heavy orientation — answer from the monitor's standing
    summary + MEMORY. Reply, and if the ask needs work, dispatch a solution
    thread and drop steering into the mailbox for the next progress pass.
    Never holds a long interactive session — dispatch-and-return.
    ("detached" form = this same process with no terminal = what lfd used to be.)
```

**Observability = capture streams, not parse tmux.** A worker has two roles that
are *independent*: a **tmux handle** so a human can attach and steer, and a
**captured out/err stream** so the monitor can summarize. batch-mode `lf -b`
emits clean stdout (no escape codes / pane redraws), so the monitor summarizes
real text — that's why "don't parse tmux, but capturing the stream is fine"
holds: tmux scrollback is dirty, batch stdout is clean. The monitor *only* ever
reads the tee'd stream logs; tmux stays a launch/attach detail, never an
observability dependency. Mechanical fork (pin at impl): `tmux pipe-pane -o` to
get both handle + stream from one launch, or run passes headless `-b` with
stdout→file and treat tmux as an optional viewer. Both work.

**No beacons — the monitor just has the streams.** Workers report nothing; they
run and narrate normally, the supervisor tees their out/err, and the monitor
reads those logs. That's the whole input.

**The monitor is a summarizer/judge, not a pipe.** Raw streams in → a judge
distills what's *relevant* → that goes to chat (+ a standing summary chat reads
for "how's it going?"). This is where "relevant details" is actually decided.
*Distinct from the killed `evaluate`:* that judged **loop control** (done /
stalled — cut, the loop just repeats); this judges **output relevance for chat**
(what's worth showing a human). Different job, kept.

Because the supervisor is non-terminating *and* serves chat, it must be
**concurrent**: it can't block on progress stdio the way the shipped `loop.rs`
does. Passes run as tracked child workers (tmux handle + stream log), leaving the
foreground free to run the cron scheduler, the monitor, and chat. The shipped
blocking loop is a stepping stone, not the shape.

**Terminal-first dependency line.** Progress + crons need *nothing* external —
they just launch tmux sessions on a trigger, hosted by `lf wave` itself. Chat is
also hosted by `lf wave` (it's already a live server), so it needs no separate
daemon either. The only thing that pulls in more (lfd-owned-identity, network
reach) is exposing that chat API to a *remote* client — a phone or Concerto.
Same artifact, exposed later.

### Legacy framing (superseded)

The rest of this doc predates the `lf wave` rename and the "no lfd" direction.
Where it says `lf loop`, read `lf wave`. Where it frames chat/crons as living in
a separate lfd daemon or a **file-based mailbox**, that's retired: chat is an
in-process API from day one, and lfd is an absorb-target, not a dependency. The
**doctrine, two-tier memory, and `<lf:pass-result>` sections below stand as-is** —
they're about what happens *inside* a pass, which the reshape doesn't touch.

## Architecture: concurrent loops sharing MEMORY

A Wave is **not one loop**. It's concurrent loops at different cadences, all thin
conductors coordinating through **MEMORY** (the shared brain). No single loop
holds a long session — the only continuity is MEMORY.

```
Wave = concurrent loops sharing MEMORY:

  PROGRESS LOOP (the engine of forward motion):
    loop: run `lf goal -b`  →  wait for it to finish  →  repeat immediately
    gated ONLY on the inner lf finishing; no timer, no delay between passes.
    each pass = one conductor pass (doctrine below): orient (from MEMORY) →
    pick highest-value blocker → dispatch lower-level work → update MEMORY.
    emits/streams progress updates ───────────────┐
                                                   ▼
  CHAT LOOP (human-facing, independent, concurrent):
    on human message: skip heavy orientation — go on what's in MEMORY + the
    streamed progress. Answer, dispatch a solution thread if the ask needs work,
    drop steering into MEMORY/mailbox for the progress loop. Never holds a long
    interactive session of its own — it dispatches solution threads and returns.

  CRONS (scheduled maintenance — keep MEMORY fresh):
    orient-daily · scan-changes · rebase · …  → refresh MEMORY out-of-band
```

**Two loops, separate on purpose.** The progress loop grinds (gated on inner `lf`,
else repeats at once). The chat loop is independent, so a human is answered
*without waiting* on a progress pass — responsiveness is decoupled from pass
duration. Progress **streams updates into chat**, so "how's it going?" is
answered from live progress + MEMORY, not a cold read.

**No long-maintained session anywhere → stuck is structurally impossible.**
Progress = tight bounded `lf goal -b` passes. Chat = dispatch-and-return. Crons =
thin scheduled passes. No context is ever held long enough to rot. That's the
real payoff of loopflow owning the loop: not one better loop, but a system where
no single context is load-bearing.

**The progress pass is light on orient.** Crons already refreshed MEMORY, so the
pass skips cold re-orient and goes straight to pick-blocker → dispatch.
Orientation is a background function of the system, not a tax on every pass.

Maps to existing infra: `cron.rs` is a first-class trigger source, waves carry a
`crons` field, and `wave/goals/4-vsm-standing-loops.md` already frames the Wave
as a viable system with regulatory loops at multiple timescales. This *is* that,
concrete.

### One mailbox, two message kinds

A user message is either pure **chat** ("how's it going?" → reply from MEMORY +
streamed progress, don't touch the plan) or **steering** ("do X first" → reply
*and* drop a directive into MEMORY/mailbox that the next progress pass picks up).
The chat loop classifies which. One channel.

## RESOLVED: rolling window + two-tier memory

"One continuous agent" = a **rolling window of chat context** that loopflow owns.
Not fresh-from-scratch (there's continuity), not unbounded-growing (can't rot).
The point is loopflow — not the vendor — decides what stays in the window and
what falls off. That's what cures stuck instead of relocating it.

Single `lf` run per pass **collapses** respond + plan + act into the run itself.
Loopflow only owns the *between-runs* machinery: drain the mailbox, append to the
window, and **stream the pass's relevant details into chat**. The model is
stateless per pass but reads both memory tiers as input — continuity without
holding the rotting transcript.

**It's just a run loop.** No `evaluate`, no done-detection gate, no
status-as-nervous-system. The loop repeats passes until you stop it (STOP file /
Ctrl-C) or steer it via chat. loopflow does *not* second-guess the pass with a
judgment function — the intelligence is in the pass and in the human watching the
chat stream. The one thing loopflow must get right between passes is **surfacing
what happened into chat**, so the wave is observable and steerable live.

```
loop(goal):                                     # just runs; stopped by STOP / Ctrl-C
  window = []                                    # rolling recent context (full, v1)
  memory = load(MEMORY.md)                       # durable distilled state
  loop:
    inbox = drain_mailbox()                       # steering from chat
    seed  = render(goal, memory, window, inbox)   # goal + durable + rolling + steering
    run   = lf_run(seed)                          # ONE bounded pass = respond+plan+act
    window.append(run.summary)                    # + any reply it emitted
    stream(run → chat)                            # ← the real work: relevant details to chat
    # v1: no eviction (full window); no evaluate — the loop just repeats
```

**Two-tier memory:**
- **Rolling window** — volatile recent chat + run summaries. Felt continuity.
  **v1: full window, no eviction** — accumulate everything; nothing rolls off yet.
  Eviction (bound by TOKENS, or time-based as a shortcut) is a later performance
  add, safe to defer because correctness never depends on the window.
- **MEMORY.md** — durable, distilled. Aged-out turns compact *into* memory, not
  vanish. Nothing important lost; window stays bounded.

**Roll is mechanical** (loopflow evicts by budget). Updating MEMORY is part of
what the single `lf` run is *asked* to do — it already has the window in front of
it, cheapest place to distill. **One harness call per pass, zero bookkeeping
calls.**

**Eviction is purely performance-driven.** MEMORY (+ repo/roadmap/state) is the
source of truth; the rolling window is just a **hot cache** of recent context so
the agent doesn't re-read MEMORY cold every pass. You evict *only* because a
bigger window = slower/costlier runs — never because old context is wrong or must
be forgotten. **Invariant: correctness never depends on the window.** That's why
eviction can be dumb (time/token) with zero risk — dropping a window entry can't
lose information (it's already in MEMORY or recoverable).

## The inner-loop prompt (the crux)

The high-level loop agent is a **conductor, not a player**. Its job is to
orient, then create lower-level work through the `lf` API (flows, steps,
sub-runs, sub-waves) — it does not solve substantial work itself. The **only**
thing that inverts vs. today's `LOOPFLOW_OPERATING_PROMPT` is *loop ownership*:
old prompt = "keep dispatching until done" (model owns loop); new prompt = "do
one orient-to-action pass and stop" (loopflow owns loop). The dispatch-and-
measure discipline stays.

**MEMORY is the orientation cache.** Vision, in-flight map, research findings,
"has the world changed" — all expensive to derive cold. They're not derived cold
each pass; they live in MEMORY and each pass refreshes only what's stale. This is
what lets the doctrine be rich *and* the passes stay bounded/fast. MEMORY is the
conductor's standing model of the goal and the world, not just a done-list.

**Assembled seed each pass:**

```
<lf:loop-pass>    operating doctrine (below)
<lf:goal>         durable objective — GOAL.md body
<lf:memory>       source of truth + orientation cache — MEMORY.md
<lf:window>       recent context — hot cache, may be trimmed
<lf:inbox>        pending human messages this pass (may be empty)
<lf:in-flight>    work dispatched earlier, still running or just finished
<lf:budget>       available workers / token budget for this wave
<lf:goal-context> flows available · roadmap handle · metrics
```

**Operating doctrine (`<lf:loop-pass>`).** Spine is a value chain:
**clarify → real user wins → what blocks them → ruthlessly prioritize.**

> You are **one pass** of a loop loopflow drives. Not the loop — loopflow calls
> you again after you exit. One orient-to-action pass, then stop.
>
> **Clarify first.** If anything you'd need to know the *real user win* is
> missing or ambiguous, clarify before charging in. Attached to a human → ask
> (in `<lf:reply>` / raise attention) and wait. Headless → make the call, log
> the assumption to `scratch/questions.md`, keep moving. Never build confidently
> on a guessed goal.
>
> **Orient** — mostly cached in `<lf:memory>`; refresh only what's stale.
> - **User wins.** What real-world wins, for actual users, does `<lf:goal>`
>   serve? Anchor on outcomes, not proxy metrics.
> - **In-flight.** Read `<lf:in-flight>` — dispatched, done, stalled. Pull
>   finished work in and reconcile it.
> - **External scan.** Has the world moved the goal — main advanced, deps
>   changed, a sibling wave landed something, requirements shifted? If the ground
>   moved, update MEMORY.
> - **Research.** For hard unknowns, investigate before committing. Cache
>   findings in MEMORY so no future pass re-derives them.
>
> **Act.**
> - **Find the blockers.** What's actually standing between us and those wins?
> - **Ruthlessly prioritize.** The single highest-leverage blocker to the biggest
>   win. Kill everything else this pass.
> - **Make sure it gets done.** Dispatch through the `lf` API, or confirm
>   in-flight work already covers it and is progressing.
> - **Scale to budget.** Big budget + lots to do → fan out parallel subworkers /
>   sub-waves, one per independent track. Small budget → one clean thread. Match
>   breadth to `<lf:budget>`.
>
> Then update MEMORY (user wins, research, in-flight map, next, blockers), emit
> `<lf:pass-result>`, and stop.
>
> Conductor, not player — direct work, don't do it yourself. Never declare done
> to escape a hard step; report `blocked`.

The **clarify gate** uses the mailbox in the *raise* direction — the conductor
doesn't only receive steering, it can post a question and (interactive) block on
it. Headless falls back to `scratch/questions.md` per the house rule.

**The pass's closing summary (part of its own stream):**

```
<lf:pass-result>
integrated: finished work pulled in this pass
dispatched: lower-level work kicked off (with run ids)
blocker:    if blocked, why
next:       what the next pass should tackle
metric:     any metric moved
</lf:pass-result>
```

Not a beacon, not a separate channel to chat — just the pass's **closing summary
line inside its own out/err stream**, which the monitor reads like any other
stream content. Two readers, both passive: it's the natural `run.summary` to
append to the window, and a tidy end-marker for the monitor's summarizer. Keep it
light — the pass emits it and stops; nothing gates on it. (Dropped `status:` —
with no `evaluate`, a progressed/stalled/done enum has no reader.)

**Single-source the doctrine.** Old `LOOPFLOW_OPERATING_PROMPT` (flow.rs:161-191)
+ removed LOOPFLOW.md dev manual **converge into one document** — the loop-pass
doctrine, authored once, woven into every pass. Not two drifting copies of "how
to operate a wave."

## What the map changed

Current architecture (all Rust under `rust/loopflow/`; `lf goal` = clap):

- **The daemon already has a dumb version of this loop.**
  `lfd/triggers/loop_ticker.rs` ticks every 5s: if a wave is idle and under
  `max_iterations`, spawn a **fresh** `lf` activation running `primary_flow`.
  Continuity is NOT preserved across activations — each is a clean session.
  So today = "loopflow owns a re-launcher on a timer." Jack's vision = "loopflow
  owns a stateful run loop that carries a window + memory across passes and
  streams each pass into chat." The gap is `state`/window threading + `drain` +
  the chat stream, not building a loop from zero and not a smart `evaluate`.

- **The mailbox is half-built and dormant.** `ChatMessage` + `ChatMemoryBlock`
  types, store methods, migrations 007/008, DTOs (`ChatMessageDto` etc.) all
  exist — with **zero HTTP routes** and constructors never called. Unwired
  scaffolding. This is `drain_mailbox`'s data layer, already in the schema.
  WS is receive-only (inbound client msgs validated but routed nowhere).

- **`act` today = a bounded `lf` child.** Daemon path is `lfd → lf (headless)
  → claude/codex`. Inner unit is `lf <flow> -b -w <wave>`. So `act` = "bounded
  burst" (model works inside one flow run, then exits) — NOT fine-grained
  one-turn interposition. We reuse this, we don't rebuild the vendor message loop.

## API evolution (the "api may need to evolve" point)

The real new surface is **stream capture**: the supervisor must tee each bounded
`lf -b` worker's out/err to `wave/<name>/streams/<run-id>.log` so the monitor can
read it. Workers surface nothing structured — they just narrate, and a
`<lf:pass-result>` closing summary rides along in that same stream. Today
`advance_branch` reads run *completion* but doesn't retain the run's output
stream; that's the gap. No structured-result-to-a-consumer API is needed, because
no consumer parses it — the monitor summarizes free text.

## Dependency: lfd is an absorb-target, not a requirement

The end state has **no lfd**. `lf wave` hosts its own runtime — pass launcher,
cron scheduler, chat API — and today's `lfd/*` migrates *into* it:

- **`lfd/triggers/{loop_ticker,cron}.rs`** — the dumb re-launcher + scheduler.
  The pass launcher's trigger policies (repeat-on-finish, scheduled) are the
  stateful successor. Absorb, don't call.
- **Mailbox DTOs (`ChatMessage`/`ChatMemoryBlock`) + HTTP/WS routes** — chat's
  data + transport, hosted in-process by `lf wave`. The **file-based mailbox
  shortcut is retired**: chat is an API from day one.

The only thing that still pulls in more is exposing chat to a **remote** client
(phone / Concerto): that needs lfd-owned-identity (from waves-outward) and network
reach. Local terminal use needs none of it — `lf wave` is the whole runtime.

## Wave shape (this branch)

**Scope call: all four arms land this branch.** Cron and chat aren't new builds
— both were **built before and mostly worked**; they went dormant only for lack
of a product vision to serve. This design *is* that vision. So the work is
**revival + rewiring**, not greenfield: bring the existing code back (from git if
removed), point it at the `lf wave` runtime, keep what worked, drop what didn't.
Do **not** rip out lfd wholesale — that's its own wave; absorb incrementally.

1. **`lf wave` supervisor + progress arm** — foreground, non-terminating,
   concurrent. Launches bounded worker passes (repeat-on-finish), each with a
   tmux handle + a tee'd out/err stream; replaces the shipped blocking `loop.rs`.
   The loop just repeats — no `evaluate`.
2. **Monitor arm** — read the workers' stream logs, run a summarizer/judge over
   them, forward relevant details + a standing summary into chat.
4. **Cron arm** — revive `lfd/triggers/cron.rs` + `loop_ticker` as the scheduler
   over the wave's `crons` field, launching the same worker passes.
5. **Chat API** — revive the dormant mailbox (`ChatMessage`/`ChatMemoryBlock`
   DTOs, migrations 007/008, WS-inbound scaffolding), now hosted in-process by
   `lf wave`. Chat answers from the monitor's standing summary + MEMORY, drops
   steering into the mailbox.

**Later (not this branch):** remote chat UI (Concerto/mobile), behind
lfd-owned-identity + network reach.

## Open questions

- **MEMORY write discipline** — *deferred to its own roadmap task; do the simple
  thing for now.* Concurrent arms can all touch `MEMORY.md`. v1 = single writer:
  **only the progress pass writes MEMORY; chat and crons append to a mailbox file
  the progress pass drains.** One writer, no locks, matches "steering drops into
  the mailbox." Don't build real concurrency control here — revisit as a roadmap
  item if the simple version bites.
- **Rolling window — DECIDED: build it, full window for v1 (no eviction yet).**
  The window accumulates; nothing rolls off at first. Safe because correctness
  never depends on the window — eviction (token/time) is a pure performance add,
  bolted on when passes get slow, not needed for v1.
- **The monitor's summarizer/judge.** Mechanism is settled (read stream logs →
  judge relevance → forward to chat). Open: its **cadence** (tick every N sec? on
  stream-append?), how it decides "relevant" (prompt-only judge over recent
  stream delta), and cost — it's an LLM pass per tick, so it needs a cheap
  trigger, not a hot spin. This is the actual crux of the branch.
- **Stream capture mechanics.** `tmux pipe-pane -o` (keeps the attachable handle)
  vs headless `-b` with stdout→file + tmux as optional viewer. Pin at impl.
- **Cron scheduler home**: internal timer in the `lf wave` process (coherent,
  terminal-first) vs absorbing `lfd/triggers/cron.rs` as-is.
- Does `lf wave` grow management siblings (`lf wave list/new/stop`), and is
  running it bare `lf wave <name>` or `lf wave run <name>`? (Leaning bare.)
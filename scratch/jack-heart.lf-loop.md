# lf loop — loopflow owns the outer loop

## What to build

Add `lf loop`: the **one place** loopflow introduces a custom harness. `lf loop`
owns the *outer* loop deterministically; every other `lf` surface (including
`lf goal`) just runs a bounded vendor-harness pass.

**`lf goal` / `/goal` stays, untouched** — people use it and `lf loop` must not
get in the way. `lf loop` is *additive*: it **reuses `lf goal -b`** (headless) as
its inner pass. So `/goal` is both the human's interactive command AND the async
unit `lf loop` dispatches.

Today the *model* owns the loop — `/goal` interactive seeds a session and the
model is *instructed* to keep going (`LOOPFLOW_OPERATING_PROMPT`) until metrics
say done. That loop gets stuck: declares victory early, spins, loses the thread.
`lf loop` reclaims the *outer* loop for loopflow and runs `lf goal -b` as bounded
async passes underneath.

> "we just do it in our language" — the loop logic is loopflow's, not rented
> from the vendor's agent loop.
> "each inner loop does `lf goal -b` or whatever"
> "I dont want to get in the way of people using /goal ... we use /goal for an
> async process"

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
Loopflow only owns the *between-runs* machinery: drain mailbox, roll window,
compact to memory, decide done. The model is stateless per pass but reads both
memory tiers as input — continuity without holding the rotting transcript.

```
loop(goal):
  window = []                                  # rolling recent context
  memory = load(MEMORY.md)                      # durable distilled state
  while not done:
    inbox = drain_mailbox()
    seed  = render(goal, memory, window, inbox)  # goal + durable + rolling + steering
    run   = lf_run(seed)                          # ONE bounded lf run = respond+plan+act
    window.append(run.summary)                    # + any reply it emitted
    window, memory = roll(window, memory)         # evict oldest → compact into memory
    done  = evaluate(goal, memory, run)
```

**Two-tier memory:**
- **Rolling window** — volatile recent chat + run summaries. Felt continuity.
  **Bounded by TOKENS** (token semantics + units), not message count.
  v1 shortcut acceptable: **time-based eviction** (keep last T / roll on a timer)
  before precise token accounting lands.
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

**Structured result loopflow's `evaluate` reads:**

```
<lf:pass-result>
integrated: finished work pulled in this pass
dispatched: lower-level work kicked off (with run ids)
status:     progressed | stalled | blocked | done
blocker:    if blocked, why
next:       what the next pass should tackle
metric:     any metric moved
</lf:pass-result>
```

`status` is the loop's nervous system. `done` is a *signal*, not a command —
`evaluate` weighs it against metrics before believing it. `stalled` = a pass that
neither integrated nor dispatched anything useful → clean stuck-signal; repeated
`stalled` triggers loopflow intervention (nudge / re-seed / raise attention),
never a silent grind.

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
  owns a stateful controller." The gap is `state` threading + a real
  `evaluate`/`drain`, not building a loop from zero.

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

For `act` to feed `evaluate`, a headless `lf` run must return a **structured
result** the controller can read — what got done, did it stall, what's blocked —
not just "it ran." Today `advance_branch` reads run completion but not a rich
state delta. New surface: bounded `lf` unit returns structured state.

## Dependency on waves-outward

- **Terminal-first `lf loop` needs NONE of it.** Controller runs in-process,
  `act` spawns bounded `lf` children, `state` + mailbox live in wave-home on
  disk (file-based STEERING). Ships on this branch alone.
- **Mobile chat needs lfd + lfd-owned-identity** (not started on waves-outward):
  wire the dormant chat DTOs → mailbox lives in lfd; WS inbound routing → live
  steering; `state`/ctx needs an lfd-owned home a phone can reach.
  Same artifact, exposed later.

## Likely wave shape (TBD — sizing pending)

1. Terminal `lf loop`: controller + plan/act/evaluate + file mailbox, `/goal`
   render reused, `act` = bounded `lf` child returning structured result.
2. Wire the mailbox in lfd (chat DTOs + routes) — steering from a client.
3. Mobile/Concerto chat UI (behind lfd-owned-identity).

## Open questions

- Continuous ctx vs fresh-seeded state (the fork above).
- Is `lf loop` an in-process terminal controller, or does it drive the lfd
  daemon loop (evolve `loop_ticker` to be stateful)? Terminal-first leans
  in-process.
- `evaluate`: mechanical (metrics/no-progress counter) or fresh bounded judge?
- Loop-paced chat only, or an instant-responder escape hatch for mid-`act`?
- Does `lf goal` get removed, or alias to `lf loop`?
```
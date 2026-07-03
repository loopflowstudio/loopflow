---
priority: urgent
---

# lf loop — the progress loop (loopflow owns the outer loop)

**Finish line:** `lf loop <wave>` runs a progress loop that fires `lf goal -b`,
waits for it to finish, and repeats immediately — driving a Goal forward
unattended without relying on the model's own (stuck-prone) goal loop.

## Context

Today the *model* owns the loop: `/goal` seeds a session and is told to keep
going until metrics say done. That loop gets stuck — declares victory early,
spins, loses the thread. `lf loop` reclaims the *outer* loop for loopflow and
runs `lf goal -b` as bounded async passes underneath.

`lf loop` is the **one place** loopflow introduces a custom harness. Everything
else — including `lf goal` — just runs a bounded vendor-harness pass. **`lf goal`
/ `/goal` stays untouched**; `lf loop` is additive and reuses `lf goal -b` as its
inner pass. Full design: `scratch/jack-heart.lf-loop.md`.

Terminal-first: needs nothing from the waves-outward lfd-owned-identity work.

## Shipped (this branch)

- **`lf loop <wave>` command** — the outer-loop skeleton: spawns
  `lf goal <wave> --once` per pass, gated on the pass finishing, repeats
  immediately; Ctrl-C or `wave/<wave>/STOP` ends it; failed passes cool down.
  loopflow owns the outer loop today. (`rust/loopflow/src/lf/commands/loop.rs`.)

## What to shape (remaining)

- **Headless passes:** make `lf goal -b` run non-interactive (route the goal
  prompt through the headless agent path) so the loop runs truly unattended;
  today each pass is an interactive `lf goal --once`.
- **The progress loop:** `lf loop <wave>` → run `lf goal -b`, gate on finish,
  repeat immediately (no timer between passes). Ctrl-C / a stop file ends it.
- **Two-tier memory:**
  - **MEMORY.md** — durable source of truth + orientation cache. The `lf goal`
    pass updates it as part of its work.
  - **Rolling window** — hot cache of recent context, threaded into each pass,
    bounded by tokens (time-based eviction acceptable as v1). **Eviction is
    purely performance-driven** — correctness never depends on the window.
- **The conductor doctrine** as `lf goal`'s single-sourced operating prompt
  (converge the old `LOOPFLOW_OPERATING_PROMPT` + the removed LOOPFLOW.md manual
  into one): one pass — clarify-first → real user wins → blockers → ruthless
  priority → dispatch through the `lf` API → scale to budget → update MEMORY →
  stop. Conductor, not player. Doctrine text is in the design doc.
- **Structured result:** each `lf goal -b` pass emits `<lf:pass-result>`
  (integrated / dispatched / status / blocker / next / metric). loopflow's
  `evaluate` reads `status`; a repeated `stalled` streak → intervene (nudge /
  raise attention), never a silent grind.
- **File-based steering mailbox** drained at the top of each pass (STEERING in
  wave-home). The lfd-backed mailbox is item 2.
- **API evolution:** `lf goal -b` must return a structured result the controller
  can read — grow the headless goal path to emit `<lf:pass-result>`.

## Done when

- `lf loop <wave>` drives a real Goal for ≥10 consecutive passes unattended,
  each pass dispatches bounded work and updates MEMORY, and a `stalled` streak
  triggers an intervention instead of grinding. `lf goal` interactive is
  unchanged.

---
priority: medium
---

# Execution engine

**Finish line:** An `approved` proposal decomposes into `ready` queue items, and
the assess-first loop dispatches a worker that lands one small autonomous item
within a blast-radius budget. Reduce stops being a hand-driven study tool and
becomes a loop that reads its own state and moves.

## Context

By the time this item is live, the proposal spine (item 2) has proven a proposal
can reach `approved`. What's missing is the machinery that turns an approved
design into shipped reductions without a human driving each step — the "hands"
of the meta-wave profile. This is milestone 3 of the arc.

The durable-state model already exists as directories under `wave/architecture/`:

```
analysis/    # study output; each file carries the HEAD it was computed against
proposals/   # draft → prototyped → proposed → approved → queued → done|rejected
queue/       # ready → in-flight → done|blocked
```

The arc is legible from that state. This item builds the loop that reads it.

## What to build

### The assess-first loop

The loop is a **priority function, not a state machine.** All three kinds of work
(study / propose / execute) are always live. Each iteration's first act is to
assess — read durable state, locate the wave in its arc, take the single
highest-value move. Top-down, take the first move that applies:

1. **Unblock** — a dispatched worker is blocked → resolve or record the blocker.
2. **Dispatch** — `ready` queue items exist and there's worker capacity → launch.
3. **Decompose** — an `approved` proposal has no queue items → break it into
   `ready` items.
4. **Surface & park** — a prototype is done and its proposal is `proposed` →
   present it for the human gate, then keep going. Never block the loop.
5. **Study** — analyses are stale past threshold (HEAD drifted well past their
   freshness marker) → refresh them.
6. **Hunt** — nothing pressing → study a new area or draft the next proposal.

### The three signals it reads first

Before choosing a move, the loop reads a small state vector:

- **How much info is gathered** — analysis coverage and depth. Thin → keep
  studying before proposing. Rich → earned the right to propose/execute.
- **How much is in flight** — dispatched-worker load. Saturated → don't dispatch;
  consolidate or unblock. Idle → dispatch or open a front.
- **What landed recently** — churn since the analyses' HEAD marker. High drift →
  the map is stale; re-study before acting on it.

### Decomposition + dispatch + budget

- Decompose an `approved` proposal into queue items small enough for a worker to
  finish and verify (the proposal may be large; gate is on design, not diff).
- Dispatch a worker against a `ready` item; move it `ready → in-flight → done`.
- Enforce a **blast-radius budget** on autonomous items. The gate is design
  agreement (item 2's territory); the budget is the safety rail for changes that
  embody *no* design decision — dead code, mechanical dedup, rename/move. Track
  % of autonomous changes that stay inside budget.

### Flow tension to resolve

`GOAL.md` currently declares `primary_flow: ship-roadmap`. The assess-first loop
above is not ship-roadmap — it's a custom priority function over reduce's own
durable state. Part of this item is deciding whether reduce runs a bespoke goal
flow or whether ship-roadmap is extended to read the analysis/proposals/queue
state vector. Pick the smaller change; note the decision in `GOAL.md`.

## Done when

- One `approved` proposal ships as N `ready` queue items.
- One small autonomous item lands within its blast-radius budget, dispatched by
  the loop rather than by hand.
- The assess pass demonstrably reads the three signals and the priority ladder to
  choose its move.

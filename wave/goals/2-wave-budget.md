---
priority: high
---

# Wave spend budget (first-class)

**Finish line:** A Wave carries a first-class `spend_cap`, accounted in real time
as the loop burns tokens; crossing it pauses the wave and surfaces a block to the
human.

## Context

A Wave that loops 24/7 racks up open-ended agent spend. The existing guardrail
(`scripts/check_monthly_spend.py` + `deploy/budget.yaml`, $100/month off the
Mercury bank feed) is **org-level, coarse, and after-the-fact** — it can't stop
a single runaway loop in real time. Wave budgets are the finer grain that
partitions the org ceiling.

**Naming:** "budget" still means deploy spend (`deploy/budget.yaml`). (The old
prompt-context `DEFAULT_CONTEXT_BUDGET` was removed with the docs-flag work —
context is now measured, not trimmed.) Call the wave field **`spend_cap`** to
disambiguate from deploy spend.

## What to shape

- **Field on the wave data structure:**
  ```rust
  pub struct SpendCap {
      pub rate: Money,            // e.g. $/day or $/month
      pub per_iteration: Money,   // ceiling for one pathological iteration
  }
  pub struct Wave {
      // ...
      pub spend_cap: Option<SpendCap>,
  }
  ```
- **Real-time accounting:** sum agent-run cost (model + tokens) across the wave's
  iterations. Today `agent.rs` tracks *turn* budgets, not dollars — token→cost
  accounting likely needs building.
- **At-limit behavior:** pause the wave + block→human (consistent with the
  existing $100 "stop for approval" gate). Reversible work keeps moving; crossing
  the line stops.
- **Chord rollup:** the parent's `spend_cap` is the ceiling; children draw from
  it. A child can't exceed the parent's remaining headroom.
- **Surfacing:** spend-to-date and headroom show in the Concerto dashboard
  (item `3-concerto-looping-sessions`).

## Open question (unresolved — Jack flagged the ambiguity)

How much budget machinery is **built into loopflow** vs **written by users**?

- **Build it in:** as the release wave matures deployment + budget tracking,
  fold that into loopflow as a first-class part of the loop — `spend_cap`,
  accounting, and the block→human pause ship with the runtime. Pro: a 24/7 loop
  is dangerous without it; safety shouldn't be opt-in. Con: more surface in the
  core; couples loopflow to a cost model.
- **Encourage self-authoring:** keep the core thin; give people the hooks
  (cost-per-iteration signal, pause/block primitive) and let them write their own
  budget goal/step. Pro: fits loopflow-as-language; users own their policy. Con:
  every user re-implements safety; easy to forget and get burned.

Lean (not decided): ship a *minimal* hard `spend_cap` + block→human in the core
(safety floor nobody should skip), and expose the cost signal + pause primitive
so richer budgeting can be written as goals on top. Resolve at build time.

## Done when

- A Wave with a small `spend_cap` runs, accrues real cost per iteration, and
  pauses with a human block when actual or projected spend crosses the cap; a
  two-level chord enforces the parent ceiling against the sum of children.

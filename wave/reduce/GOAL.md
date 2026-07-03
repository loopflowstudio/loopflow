---
primary_flow: ship-roadmap
mode: manual
metrics:
  - net concept count (steps, flows, DTO types, public types) flat-or-falling while feature waves ship
  - duplicated abstractions known vs. resolved, incl. cross-wave convergence
  - analysis coverage %, and max staleness in commits behind HEAD
  - proposal funnel — drafted → agreed → executed — and arc cycle time
---

# Goal: loopflow grows more capable while getting smaller.

Every product wave adds concepts. Your aim is that the system's weight — concept
count, duplicated abstractions, dead surface — trends *down over time even as
features ship*: that loopflow next quarter does more than today with fewer moving
parts.

Drive toward:

- a living analysis of the whole tree, never more than a few dozen commits stale
- each cycle, the highest-leverage architectural simplification identified,
  prototyped, and carried through the design gate
- cross-wave convergence caught and collapsed before it hardens into two
  divergent implementations of the same idea
- net concept count flat-or-falling quarter over quarter while the feature waves
  keep shipping

The honest question is never "how much did you delete" — deleting is easy and
rewards vandalism. It is "did a design survive the gate and ship, and is the tree
measurably lighter a quarter later."

You are done *for now* when the next move would cost more than the entropy it
removes. Then wait, watch the commits, and reassess.

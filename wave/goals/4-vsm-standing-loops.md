---
priority: low
---

# VSM system charters as standing chord loops

**Finish line:** The chord decides — and enforces — which of the five VSM
systems run as standing Looping Agents versus collapse to reflexes or member
waves. Whatever survives runs on its own loop against the chord's context, not
just on-demand via `lf goal --system`.

## Context

Shipped: the five VSM systems are builtin goal charters
(`engine/builtins/govern/goal/govern-{operations,coordination,control,
intelligence,identity}.md`) resolvable through `lf goal <wave> --system s1..s5`.
Each charter is a generic five-move compass (true north, drive, progress test,
reorientation, deferral) whose deferrals form a closed ring
(S1→S2/S3, S2→S1/S3, S3→S4/S5, S4→S3/S5, S5→S3/S4) — that closure *is* the
viable-system property. The `govern-*` flows are still the hands; the charter is
the head. `lf goal <wave> --system` swaps the goal body while keeping the wave's
roadmap, memory, metrics, flows, and in-flight work.

What's unresolved is the **standing** question. Right now you invoke a system
charter by hand against a chord. The open design question, deliberately left for
the running system to answer rather than pre-judged:

- **Symmetric five vs asymmetric governor.** We shipped all five charters
  symmetric. But do S1 (operations) and S2 (coordination) earn their own always-on
  loops, or do they collapse into member-waves (S1 = the leaf waves doing the real
  work) and reflexes (S2 = a fast damping response, not a deliberating agent)?
  S3/S4/S5 are the plausible standing governors; S1/S2 may not need a separate head
  at all.

## What to shape

- **Wire a system charter to run as a standing loop**, not just `--once` on
  demand — a chord-level cron or trigger that ticks one or more VSM charters
  against the chord each cycle. Sequence with the garden cycle
  (`workflows/1-daily-garden-cycle`), which already runs a scheduled s5→s2 pass
  as flows; the charter version is the goal-driven head over that hand.
- **Answer symmetric-vs-asymmetric empirically.** Run the five against the root
  chord for a stretch; watch which produce useful moves and which idle. Let the
  data decide whether S1/S2 stay as charters, collapse to member-waves/reflex, or
  drop.
- **Recursion is already free** — point a charter at the root chord or a
  sub-chord; same charter, different context. Exercise this once standing loops
  exist.

## Done when

- At least one VSM charter runs unattended on a schedule against a chord and
  produces reviewable moves across several cycles.
- The symmetric-vs-asymmetric question is resolved by observed behavior, with the
  decision (and any charters dropped or collapsed) recorded.

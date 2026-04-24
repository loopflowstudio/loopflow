---
asana_id: '1214269992184208'
---
# Governance surfaces

**Finish line:** The system-level Concerto surfaces — runboard, portfolio, calibration, beat programming, and release controls — all read from the same engine-backed model of waves, runs, attention, mutations, and schedules. No dashboard fork, no UI-only shadow state.

## Context

These surfaces live in the macOS app, but they are workflow work. They express how the engine thinks about the system:

- **Runboard** — what's happening now across waves
- **Portfolio** — what the whole system looks like at a glance
- **Calibration** — where garden output becomes a human decision
- **Beat programming** — how scheduled rhythm is composed
- **Release controls** — when and how a repo ships

If these screens invent their own data model, they drift from the actual engine. If they share the engine model, they become trustworthy.

## Daily experience

Open Concerto and the governance picture is obvious: what shipped, what is blocked, what root proposes, what cadence is running, and whether a release needs attention. Drill in anywhere and you're still reading the same underlying state.

## Done when

- Runboard, portfolio, calibration, beat programming, and release controls share one underlying model
- Garden and govern output shows up without bespoke translation logic per screen
- A reviewer can trace any UI state back to wave/run/attention/mutation data in lfd
- The surfaces help steer the system instead of merely reporting on it

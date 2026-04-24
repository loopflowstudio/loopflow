---
asana_id: '1214270114261878'
---
# Conductor surfaces

**Finish line:** The conductor's morning routine across all waves lives in one Concerto surface family: runboard (live cockpit with health + drill-in), portfolio (cross-wave / cross-repo gallery), calibration (garden-flow human checkpoint), beat composition (program a chord's rhythm). All share the same `wave / run / attention / terminal-session` stores — no parallel dashboards.

## Context

The Concerto Flows tab and repo-card portfolio shell exist. `AttentionQueueView` exists. The workspace multiplexer has a launcher pane. Runboard, portfolio-as-a-view, calibration UX, and beat composition don't yet exist as first-class surfaces.

Three altitudes, one data model:

- **Runboard (cockpit, low)** — what's happening right now: wave mode, current step, agent health (idle / running / blocked / errored), beat history, drill-in to steer
- **Portfolio (gallery, high)** — how's the whole system: wave cards with health / PR state / attention count, chord grouping, cross-wave indicators, single-repo + multi-repo scope, trend lines
- **Calibration (garden checkpoint, meta)** — dedicated UX for `wave/review`: trajectory across waves, chord's proposed mutations, trajectory notes that flow into memory
- **Beat composition (compositional)** — program chord rhythm: assign waves to beat slots, visualize play / tune / silence

## Daily experience

Morning: open Concerto, runboard is home. 10-second scan: 3 PRs shipped overnight, 1 wave blocked (yellow dot), 1 calibration checkpoint waiting. Drill into the blocked wave — see what the agent got stuck on, nudge it. Click calibration — chord has 2 proposed mutations, approve one, reject the other with a note. Close laptop.

## Done when

- Runboard shows all waves live with health + drill-in, refreshed via the WebSocket
- Portfolio works for single-repo and multi-repo scope
- Calibration checkpoint surfaces garden-flow `wave/review` with mutation approval + trajectory notes
- Beat composition lets you reorder and assign waves to beat slots
- All four surfaces read from the same underlying store — no dashboard fork

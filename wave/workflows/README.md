# Workflows

The engine. Flows, providers, daemon, chord model, governance UX, runboard.

## Vision

Everything below the Concerto UI layer. The CLI, the daemon, the providers, the flow system, the chord model, and the governance surfaces compose into one engine that defines how loopflow actually runs.

A single wave keeps this work coherent — one engine upgrade at a time, not four parallel refactors that never converge. Inside the wave, items stay focused on their area.

### Not here

- macOS UI polish (→ desktop).
- iOS read-only view (→ mobile).
- Chord-level governance questions about the root wave itself (→ root).

## Scope

- **lfd** — runtime host, shared execution model, session input, stream cursoring, regression tests.
- **model** — chord-model, VSM flows, wave discovery, scheduling, DAG/nested chords, Letta integration, wave mutation, API expansion.
- **pm** — Asana/Linear/Notion sync, dependency sync, run-lifecycle sync, Notion README sync, team-level delete / reset ops.
- **gstack** — gstack workstyle import, autoresearch, infrastructure model, company model.
- **flows** — catalog, session-state overlay, `maybe` primitive, placement tuning, Flows-view polish.
- **runboard** — wave status API, runboard UI, agent health detection, beat sequencer.
- **governance UX** — calibration view, portfolio view. Attention surfaces for the garden flow.
- **experimental** — beat synthesizer, concerto release UI.

## Risks

- Too broad to hold in one head. Roadmap priority buckets (1 urgent / 2 high / 3 medium / 4 low) are the focus tool — the wave is big, the active set is small.
- Inter-item dependencies between lfd / model / runboard / flows need explicit tracking.
- Governance UX (calibration, portfolio) is coupled to model work — surfaces depend on the underlying wave-modes and run-lifecycle landing first.

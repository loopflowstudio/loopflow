# Stimulus as CRUD Resource

## What was implemented

Refactored stimulus from an embedded field on waves to a standalone CRUD resource.

**Before:** `Wave.stimulus` was a single field (kind + cron) on the wave object. Running a wave could set the stimulus inline. Stimuli had an `enabled` boolean to toggle them on/off.

**After:** Stimuli are independent resources stored in a `stimuli` table. Waves have zero or more stimuli. Creating/removing stimuli uses dedicated endpoints. Existence in the table = active (no `enabled` flag).

## Key choices

**Separate CRUD endpoints over inline on run.** The initial commit embedded stimulus in the `run_wave` request. This was reversed in favor of `POST /waves/:id/stimulus` and `DELETE /waves/:id/stimulus/:id`. Reasoning: stimuli are persistent config, runs are ephemeral actions. Mixing them conflates two concerns.

**Removed `enabled` field.** Existence = active. To disable a stimulus, delete it. This simplifies the trigger loops (no `if !stimulus.enabled` checks), the store schema, and the mental model.

**Removed `manual` and `once` from Swift Stimulus.Kind.** These weren't real stimulus types — `manual` meant "no stimulus" and `once` was just a regular run. In the new model, no stimuli = manual, and `run_wave` is a one-shot run. Only `loop`, `watch`, and `cron` remain as stimulus kinds.

**Split button UI.** StepRunner now has separate "Run" (one-shot) and auto-mode (loop/watch/schedule) buttons with a dropdown to pick mode. When an active stimulus exists, it shows that instead of the buttons.

## How it fits together

```
StepRunner UI
  ├── Run button     → POST /waves/:id/run (one-shot)
  ├── Auto button    → POST /waves/:id/stimulus (creates stimulus)
  └── Active display → DELETE /waves/:id/stimulus/:id (removes)

Rust backend
  ├── run_wave_handler     — fires a single wave run
  ├── add_stimulus_handler — creates persistent trigger
  ├── remove_stimulus_handler
  └── list_stimuli_handler

Triggers (loop_ticker, watch, cron)
  └── Read stimuli table, fire runs when conditions met

Python client
  ├── run_wave()        — no stimulus param
  ├── add_stimulus()    — kind + optional cron
  └── remove_stimulus() — by id
```

Wave DTO now includes `stimuli: Vec<StimulusDto>` so clients always see active stimuli without a separate fetch.

## Risks and bottlenecks

**Migration of existing stimuli.** SQLite migration inserts rows into `stimuli` from the old `stimulus_kind`/`stimulus_cron` columns on `waves`. Postgres migration drops `enabled`. Both are forward-only — no rollback path.

**Multiple stimuli per wave.** The data model supports it, but the UI only shows `stimuli.first`. If someone adds multiple via API, only the first shows in Concerto. This is fine for now — the primary use case is one stimulus per wave.

## What's not included

- No `PATCH /waves/:id/stimulus/:id` (update in place) — delete and recreate instead.
- No stimulus history/audit trail.
- Python `Stimulus` model gained an `id` field but no separate list endpoint in the Python client (can use `wave.stimuli` from the wave response).

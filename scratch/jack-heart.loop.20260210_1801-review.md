# StepRunner: auto-run modes and stimulus plumbing

## What was implemented

StepRunner's execution UI was redesigned from a flat stimulus picker (once/loop/watch/cron pills) to a split-button pattern: a primary "Run" button for one-shot execution and a secondary auto-mode button with a dropdown to switch between Loop, Watch, and Schedule modes. The backend (Rust + Python) was extended to accept a `stimulus` field on the run-wave endpoint, enabling the UI to persist the chosen auto-run mode when starting a wave.

## Key choices

**Split button over pill picker.** The old design gave equal visual weight to all four stimulus types. The new design makes one-shot runs the primary action (most common) and groups continuous modes behind a single auto button with a mode dropdown. This reduces visual noise while keeping all modes accessible.

**`AutoMode` enum private to StepRunner.** The enum maps between the view's three auto modes (loop/watch/cron) and `Stimulus.Kind`, keeping the UI model separate from the data model. `once` and `manual` are not auto modes — they're handled by the Run button directly.

**`isSendingRun` loading state.** Replaced the old pattern of checking `wave.status == .running` to determine button state. The new flag covers the gap between tap and server response, preventing double-sends. Both buttons share the `buttonsDisabled` computed property.

**Stimulus upsert on run.** The Rust handler creates or updates a stimulus record when the request includes one. `enabled` is set to `false` for `.once` stimuli so they don't trigger the scheduler loop.

## How it fits together

`StepRunner` → HTTP POST `/v0/waves/:id/run` with `{flow, stimulus: {kind, cron?}}` → `run_wave_handler` updates the stimulus record and spawns the run. The Python API (`run_wave`, `Client.run_wave`) passes the stimulus through unchanged via `model_dump(exclude_none=True)`.

## Risks and bottlenecks

- **No loading indicator on auto button.** When the auto button triggers a run, only the Run button shows a spinner. The `buttonsDisabled` flag disables both, so there's no double-send risk, but the visual feedback is on the wrong button. Minor UX issue.
- **Stimulus upsert races.** If two requests arrive simultaneously for the same wave, the list-then-update pattern could produce duplicates. Low risk in practice — Concerto sends one request at a time.

## What's not included

- No new tests for the stimulus upsert logic in the Rust handler. The existing wave run tests cover the happy path; the stimulus upsert is additive.
- The `RunWaveStimulus` struct is request-scoped and doesn't need to match the full `Stimulus` storage model.

---
requires: existing code
produces: code changes
---
Close the loop: show the wave agent its own in-flight work (open dispatches + PR
state) so it re-measures reality and doesn't double-dispatch.

Design context: `scratch/waveagent-sessions.md` ("Close the loop: PR state feeds
re-measure"). Additive only — do not touch triggers, do not delete anything, do not
build a land trigger.

## Goal

Today the wave loop renders GOAL + MEMORY + flows/roadmap/metrics, but it has no view
of what it already dispatched. Dispatched work lives in open PRs (not in main), so the
agent can pick a roadmap item whose PR is still in flight and dispatch it again. Feed
the wave's in-flight runs — each dispatch's `task`, `flow`, `status`, and PR state —
into the render context so the read step sees in-flight work.

## Workflow

1. **Context field.** In `rust/loopflow/src/engine/flow.rs`, add to
   `GoalRenderContext` a field `in_flight: Vec<InFlightDispatch>` where
   `InFlightDispatch { task: Option<String>, flow: String, status: String, pr_url: Option<String>, pr_state: Option<String> }` (a small plain struct, `Debug, Clone`, defined in this module).

2. **Render it.** In `render_goal`, add an `<lf:in-flight>` section listing each entry
   as a scannable line, e.g. `- [<status>] <flow>: <task> (<pr_state> <pr_url>)`.
   Render a clear "No work is in flight." when empty. Keep `render_goal` pure — it
   only formats what the context gives it. Update/extend the existing `render_goal`
   tests to cover a non-empty `in_flight`.

3. **Populate it.** In `rust/loopflow/src/lfd/executor/wave/mod.rs`
   `build_wave_run_command` (or its caller that has store access — the executor has
   `self.store`), query the wave's active/recent runs and map them to
   `InFlightDispatch`. Use the existing store API to list a wave's runs (grep the
   `Store`/`WaveRunStore` trait in `rust/loopflow/src/lfd/store/mod.rs` — e.g.
   `list_stack_runs`, `get_active_wave_run`, or add a small `list_active_wave_runs`
   query if none fits). Include runs that are Pending/Running/Waiting or have an open
   PR; exclude Completed/Failed/merged. Pull `pr` (`WaveRun.pr`: url + state) and
   `snapshot.task`/`snapshot.flow`/`status`.
   - Note: `build_wave_run_command` is currently a free function `(wave, run)`. If it
     needs the store, thread the in-flight list in from the async caller (which has
     `self.store`) rather than making the pure builder do I/O — pass
     `in_flight: Vec<InFlightDispatch>` as a parameter. Keep the builder testable.

4. **Tests.** A `build_wave_run_command`-level test (or the caller) showing that when
   the wave has an in-flight dispatch, the rendered prompt contains the `<lf:in-flight>`
   section with that task/flow/PR; and an empty case. Follow the existing test style in
   that module (in-memory store / fakes already used there).

## What matters

- Additive: goal/memory/flows/metrics rendering stays; `<lf:in-flight>` is a new
  section.
- The pure `render_goal` does no I/O; the executor supplies the in-flight list.
- Only genuinely in-flight work (open runs / open PRs) — not completed/merged.

## Guardrails

- Do NOT touch trigger/cron code, do NOT delete existing functions, do NOT add a land
  trigger or supervisor.
- Do NOT add wire-DTO defaults; `InFlightDispatch` is internal render state, not a
  wire type.
- `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test -p loopflow` must pass.
- Keep the diff tight and additive.

## Output

The wave loop's rendered prompt includes an `<lf:in-flight>` section of open
dispatches + PR state, with tests. The agent now re-measures reality before
dispatching.

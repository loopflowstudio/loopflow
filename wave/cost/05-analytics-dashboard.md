# 05: Analytics Dashboard

**Finish line:** Opening the analytics tab shows token trends over time, groupable by wave/flow/step/model, with a separate prompt composition view.

## What to build

A dedicated tab/view with two analytical lenses:

**Work lens** — tokens grouped by wave, flow, step, model. Time-series charts (daily/weekly/monthly). Cross-wave comparisons.

**Prompt lens** — token composition by source (docs, diff, area, system, clipboard, wave memory). Stacked composition charts. Filterable by wave/flow/step. Surfaces the "token tax" of each context source.

Period picker, grouping selector. Reads from `/usage/summary` endpoint.

This phase also populates `CostRates` in the model registry (`provider_models.rs`) — infrastructure exists from Phase 03 with all rates `None`. Fill in actual per-token prices for OpenCode Zen models. Compute `TurnUsage.cost_usd` from rates for per-token providers.

## Context from shipped phases

- Session events are immutable after session ends — caching is safe. Add when measured latency exceeds 200ms.
- `AgentRun` and `Session` are two parallel types for one concept (one agent invocation). `AgentRun` tracks process lifecycle, `Session` tracks conversation/usage. Unify or make the relationship explicit before building cross-session analytics.
- `ContextSnapshot.budget` is not yet sourced from session-level configuration. If dynamic context budgeting lands, the prompt lens should reflect it.


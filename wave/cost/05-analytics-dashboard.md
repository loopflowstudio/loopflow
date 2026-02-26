# 05: Analytics Dashboard

New Concerto surface for strategic token analysis.

## What to build

A dedicated tab/view with two analytical lenses:

**Work lens** — tokens grouped by wave, flow, step, model. Time-series charts (daily/weekly/monthly). Cross-wave comparisons.

**Prompt lens** — token composition by source (docs, diff, area, system, clipboard, wave memory). Stacked composition charts. Filterable by wave/flow/step. Surfaces the "token tax" of each context source.

Period picker, grouping selector. Reads from `/usage/summary` endpoint.

## Done when

Opening the analytics tab shows token trends over time, groupable by wave/flow/step/model, with a separate prompt composition view.

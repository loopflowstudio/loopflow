---
crons:
- flow: telemetry-daily
  schedule: 0 0 8 * * * *
pm:
  linear_initiative: 1e3d8674-6fbf-4aa8-9bee-1da0fa70d1b7
---

## Objective

Explicitly selected work is explainable from durable local context and Run evidence.
Intelligence makes initiating intent, exact submitted context, provider activity,
usage, outcomes, and observed consequences inspectable, with missing evidence
stated honestly. That evidence supports execution diagnosis and justified prompting
changes. It never selects priorities, grants execution authority, or substitutes
successful process exit for delivered progress.

## Projects

Projects and tasks live in Linear and sync into the local SQLite registry.
Projects do not own memory, cadence, or child projects.

## Bounds

- No remote telemetry, global run server, or vector-store dependency.
- No cross-machine journal replay, and no vendor memory backend (Letta and
  kin) taken on as a dependency.

## Cron

- `daily` -> audit context quality and trace coverage; if a run cannot explain
  what the model was told, why those instructions fit its situation, or why it
  behaved differently, make that the next task.
- `weekly` -> inspect one smooth, one costly, and one failed or heavily steered
  run; classify the first context or trace failure, then file or dispatch the
  smallest evidence-backed repair.

## Process

Read the accepted chapter and synced Project, then inspect the real long-lived
records and actual launch boundary before choosing a repair. Preserve exact
submitted bytes and component provenance; absent required context fails
reconstruction. Vendor-unavailable measurements stay explicitly unavailable.
Include attempts that fail before a Run exists and retain causal identities.

Infrastructure owns execution repair and control authority. Product judges the
usefulness of explicitly selected external workflows and presents shared evidence.
Intelligence improves context and readers, without a second monitoring authority or inferred
priorities. Use one serial implementation slot: capture, population accounting,
then usable reconstruction. Collect real proof windows without inventing activity,
backdating budgets, or shortening the accepted interval.

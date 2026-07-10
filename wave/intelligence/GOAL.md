---
crons:
- flow: telemetry-daily
  schedule: 0 0 8 * * * *
pm:
  provider: linear
  linear_initiative: 1e3d8674-6fbf-4aa8-9bee-1da0fa70d1b7
---

## Objective

Loopflow gets sharper from evidence. Intelligence owns the information loop
around agent work: context, trace, and evals.

The work succeeds when every prompt, context, and workflow change can point to
run evidence, and future runs measurably improve without adding remote
telemetry, a vector backend, or a hidden knowledge store.

## Projects

Projects and tasks live in Linear and sync into the local SQLite registry.
Projects do not own memory, cadence, or child projects.

## Bounds

- No remote telemetry, global run server, or vector-store dependency.
- No cross-machine journal replay, and no vendor memory backend (Letta and
  kin) taken on as a dependency.

## Cron

- `daily` -> audit context size, trace coverage, and eval freshness; if a run
  cannot explain what the model saw or why it behaved differently, make that
  the next task.
- `weekly` -> inspect recent local run evidence, choose one prompt/context
  edit that measurement justifies, and file or dispatch it.

## Process

Read the projects, then start from evidence, not vibes: read the ledger, prompt
artifact, eval result, or context boundary that should answer the question, and
only then choose work. Run the reader before trusting it — a measurement taken
against a fresh store proves nothing about the long-lived one that holds the
history, and a surface nobody has queried on real data is a surface that does
not work. Instrument a blind spot when no measurement exists; edit a prompt,
context surface, or builtin when the evidence already points. Move lessons
between builtins and repo agent docs only after real runs prove the
distinction.

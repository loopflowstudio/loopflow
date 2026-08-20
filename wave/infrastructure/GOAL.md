---
crons:
- flow: telemetry-daily
  schedule: 0 0 9 * * *
- flow: release-run
  schedule: 0 0 10 * * *
pm:
  linear_initiative: 218967b6-a760-4b7c-9a46-11d9d61a42c2
---

## Objective

Loopflow's infrastructure makes ordinary work fast, safe, and boring. It keeps
the system legible, converts failures into prevention, and turns repeated
operational friction into durable capability.

## Projects

Projects and tasks live in Linear and sync into the local SQLite registry.
Projects do not own memory, cadence, or child projects.

## Bounds

- Do not build a generic multi-product deploy platform before a second real
  product proves the shape.

## Cron

- `telemetry-daily` -> check architecture drift, local development friction,
  CI, release cadence, spend, and host health; turn the first red or flaky
  signal into focused work.
- `release-run` -> attempt one patch release after telemetry. No merged changes is
  a green no-op; an incomplete tagged release resumes from its hosted build.

## Process

Read the projects, then look for the bottleneck currently taxing real work.
Mechanical fixes and obvious automation go straight to a worker. Anything that
changes architecture doctrine, release policy, host topology, credential flow,
or the worker/wave runtime gets a scratch design first. Do not document
avoidable manual work as a workflow; delete it with code.

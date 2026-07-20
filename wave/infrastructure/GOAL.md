---
crons:
  - flow: telemetry-daily
    schedule: "0 0 9 * * *"
  - flow: release-run
    schedule: "0 0 10 * * *"
pm:
  provider: linear
  linear_initiative: 218967b6-a760-4b7c-9a46-11d9d61a42c2
  linear_team: 08c8d501-791d-4db0-a14c-15599d365955
---

## Objective

Loopflow's infrastructure keeps real waves moving. It owns the substrate around
the product: technical architecture, developer efficiency, and release
stability.

The work succeeds when the system is legible, local work is fast, and shipping
is boring. Infrastructure does not build a generic platform ahead of need; it
turns repeated friction and operational risk into system capability.

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

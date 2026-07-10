---
crons:
- flow: wave
  schedule: 0 0 8 * * * *
pm:
  provider: linear
  linear_project: 9ee88f2a-ef37-46c7-b201-d197db3ccae0
  linear_initiative: 33e774b0-ec3b-4bd6-a4f8-07676f9e897b
---

## Objective

Loopflow is the product for conducting goal-authored work across humans,
agents, devices, and machines. The product is not one interface; it is the
shared API and surfaces that make waves understandable and steerable wherever
they run: CLI, Mac, iOS, agent turns, local workers, and remote compute.

The work succeeds when a user can create a wave, understand its state, steer
it, delegate work, inspect its record, and move work across machines without
caring which process owns the machinery underneath.

## Projects

Projects live in `projects/`, one measured bet per file. Tasks live in Linear.
Projects do not own memory, cadence, child projects, or task lists.

## Cron

- `daily` -> dogfood one complete product path across CLI or app surface:
  create, understand, steer, delegate, inspect, or recover. Convert the first
  real product failure into a task under the project it proves.

## Process

Read the projects, then dogfood before guessing. Keep the API and surfaces in
lockstep: CLI, Mac, iOS, prompts, and workers should expose the same product
model, not parallel concepts. Product work starts from the user contract; when
it changes process ownership, wire shape, launch lifecycle, or distributed
execution, write the scratch design first. Visual and ergonomic rough edges can
go straight to implementation when the product contract is stable.

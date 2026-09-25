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

Loopflow reliably advances its own work and delivers verified releases. Infrastructure
owns self-hosting reliability, releases, auth, execution continuity, exact control
authority, and repository-wide architecture reduction. Recovery preserves work,
recorded decisions, history, and assigned-worktree isolation; failures are bounded,
truthful, and actionable. Public concepts and APIs remain legible, with one owner
and truth source for each responsibility.

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

Read the accepted chapter and synced Projects, then repair the demonstrated
bottleneck in selected work. Reliability owns runtime/auth/release mechanics;
Intelligence owns raw context and attempted-operation evidence; Product owns
interactive presentation and external-outcome judgment. Architecture Minimalism owns
repository-wide reduction and Task execution authority, including List's retained
safe-signaling, promotion-preservation, and exact-authority obligations.

Wave, Project, and Task remain durable Work. Long-lived implementation pursuit
and PR ownership belong to Tasks; Wave and Project operations are bounded Runs
against their durable records. Changes to architecture, releases, auth, execution,
or placement start with a design and preservation counterexamples. Keep one serial
implementation slot per Project; chapter allocation is planning guidance, not a
new runtime limit. Keep the configured release schedule and required verification.

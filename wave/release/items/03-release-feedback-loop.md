---
priority: p2
status: open
---
# Release feedback loop

Turn release automation failures into work that gets handled.

## Goal

Nightly and weekly failures surface as attention items or focused fix PRs with enough context to act quickly.

## Done when

- Failed nightly/weekly runs can be discovered from Loopflow, not only GitHub Actions.
- CI failure repair flows know which release workflow failed and why.
- Human-facing status distinguishes package verification failure, publish failure, deploy/host failure, and stale-local-copy drift.

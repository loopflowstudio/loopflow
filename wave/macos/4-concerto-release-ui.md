---
linear_id: a35cd281-86f1-4c10-85a0-20690e713965
---
# Concerto Release UI

**Finish line:** Concerto shows repo release targets and lets a human trigger a release with a patch/minor/major choice from the app.

## Context

The release machinery already exists in CLI form. `lf release <version>` runs the full workflow, and `lf op release check|notes|bump|tag|status` expose the mechanical pieces underneath it. Repo config already has a `release.targets` model in `.lf/config.yaml`; what is missing is a Concerto surface that makes those capabilities visible without dropping to a terminal.

There is no current Swift release view, model, or service. This item should start from read-only visibility plus a clear run action rather than trying to turn Concerto into a full release-config editor on day one.

## What to build

1. Add a backend surface that exposes parsed repo release targets and current release status in a Concerto-friendly shape.
2. Show release targets in repo detail with the data people actually need: target name, tag prefix, area scope, manifests/workflow.
3. Add a `Release Now` action with target + version-bump selection (patch/minor/major, and custom if the underlying command needs it).
4. Surface in-progress, empty-release, and failure states clearly enough that the operator can tell whether anything happened.
5. Link the completed run back to the resulting tag and workflow status.

## Constraints

- Start with release visibility and execution; full config editing can wait for a later item if it proves necessary
- Keep version choice human-supplied, not agent-generated
- Reuse existing release commands and config parsing instead of inventing a second release pipeline for Concerto
- Avoid a wave-level abstraction leak: releases are repo-scoped, even if the button lives near wave detail

## What this item should teach us

- Whether repo detail is the right home for release actions or if they need a dedicated operator surface
- Whether read-only release target summaries are enough for the first pass
- Which release failures need first-class UI treatment instead of raw command output

## Done when

- Concerto can display repo release targets parsed from `.lf/config.yaml`
- A human can choose a target and version bump, then run the release from the app
- Concerto surfaces release status/results without requiring terminal output inspection
- Empty-release and failure cases are legible in the UI

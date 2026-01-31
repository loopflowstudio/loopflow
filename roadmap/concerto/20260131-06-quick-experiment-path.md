---
status: todo
phase: 1
persona: concerto
order: 6
sources: [improviser]
---

# Add a quick experiment path without creating a wave

Exploration should allow a one-off action without committing to a persistent wave.

## Current

The UI assumes wave creation is required for any action.

## Problem

Improvisers must commit to naming/configuring a wave before trying a simple step. This adds unnecessary friction to exploration.

## Build

- Add a "Quick Run" entry point that runs a single step without persisting a wave
- Optionally auto-expire temporary runs so they do not clutter the sidebar

## Done when

A user can run a single step without creating a persistent wave.

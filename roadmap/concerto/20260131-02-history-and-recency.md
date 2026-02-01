---
status: todo
phase: 1
persona: concerto
order: 2
sources: [listener, ceo, conductor]
---

# Add history and recency cues

The UI should show what changed since the last check-in without opening each wave.

## Current

There is no activity timeline and no last-activity timestamps in the sidebar. Iteration counts exist but do not explain what happened.

## Problem

Returning or executive users cannot tell what changed while they were away. They must click into waves and reconstruct context manually.

## Build

- Add a recent activity panel (or sidebar section) with timestamps
- Add last-activity timestamps on wave rows
- When showing iteration counts, include a brief summary of the most recent transition

## Done when

A user can answer "what happened since I last checked?" without opening any wave detail.

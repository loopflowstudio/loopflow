---
status: todo
phase: 1
persona: concerto
order: 3
sources: [conductor, improviser, listener, ceo]
---

# Make waiting states actionable with context

When a wave is blocked, the UI should show the reason and the next action in one place.

## Current

Waiting states display a reason like "PR limit reached" but offer no direct path to resolve it.

## Problem

All personas see the blocker but not the action. Users must leave Concerto to find the PRs or decide what to do next.

## Build

- Show the waiting reason inline with a clear next action
- Link directly to blocking PRs or provide a "Review PRs" button
- Include counts such as "2/3 PRs open" to clarify what is blocked

## Done when

A user can go from "I see a block" to the resolving action in one click.

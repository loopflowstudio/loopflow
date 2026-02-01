---
status: todo
phase: 1
persona: concerto
order: 4
sources: [conductor, improviser, ceo, product-designer]
---

# Show running progress and provide a clear connect path

Running waves should show progress and offer a lightweight way to inspect or intervene.

## Current

Running state shows a spinner and "Running ship flow" with little progress detail. The only actions are Stop and Clone.

## Problem

Users cannot tell if a wave is healthy or stuck, and they cannot quickly inspect what is happening without heavier actions.

## Build

- Show elapsed time and current step (e.g., Step 2/4)
- Surface recent output or a compact activity summary
- Add a "Connect" or "Attach" action distinct from Stop/Clone

## Done when

A user can judge progress and attach to a running wave without stopping it.

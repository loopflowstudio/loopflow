---
status: todo
phase: 1
persona: concerto
order: 1
sources: [conductor, listener, ceo, product-designer]
---

# Show attention grouping and counts in the sidebar

The sidebar should make "what needs attention" obvious without clicking into waves.

## Current

Waves appear in a flat list with minimal status cues. There is no header count or grouping, and some screenshots show an empty sidebar with no structural preview.

## Problem

Both high-level and returning users cannot tell at a glance if anything needs attention. The UI requires scanning or drilling into waves, which defeats the "check-in fast" use case.

## Build

- Group waves into sections (Needs Attention, Waiting, Active, Idle)
- Add counts in section headers and/or a badge in the sidebar header
- Always show the section headers even when empty, so the mental model is visible

## Done when

A user can answer "anything needs attention?" in under 5 seconds from the sidebar alone.

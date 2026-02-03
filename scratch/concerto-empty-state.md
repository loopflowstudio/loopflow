---
status: todo
phase: 1
persona: concerto
order: 5
sources: [improviser, listener, product-designer]
---

# Empty state teaches mental model and enables quick experiments

The empty state is the first impression. It should both explain the system and remove friction for exploration.

## Problem

New users see "No waves yet" with a generic "Create a wave to start AI-powered work" description. This fails on two fronts:

1. **No mental model.** What's a wave? What will I see when I have some? The current empty state doesn't preview the eventual structure (Needs Attention, Open PRs, Active, Idle sections).

2. **Too much commitment.** The only action is "Create Wave," which implies naming something, configuring it, committing to it. Improvisers want to try a step first, not plan a wave.

## Approach

Replace the empty state with two paths:

```
┌────────────────────────────────────────┐
│          Quick Experiment              │
│  Run a step without creating a wave    │
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐  │
│  │design│ │review│ │debug │ │ ... │   │
│  └──────┘ └──────┘ └──────┘ └──────┘  │
├────────────────────────────────────────┤
│   Or create a wave for ongoing work    │
│           [ Create Wave ]              │
│                                        │
│   Waves run flows on your codebase     │
│   ┌─ Needs Attention (2)               │
│   ├─ Open PRs (1)                      │
│   ├─ Active                            │
│   └─ Idle                              │
└────────────────────────────────────────┘
```

**Quick Experiment (primary):** Step buttons that launch a step immediately on the entire repo. No wave created, no persistence. Terminal opens, step runs, done.

**Create Wave (secondary):** For users who want a persistent workstream. Shows a structural preview of what the sidebar will look like when populated.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Show StepRunner inline without wave | Would require significant refactoring of StepRunner which assumes a wave exists | More work, same outcome |
| Add "ephemeral wave" flag | Still creates persistence, clutters wave list with abandoned experiments | Complexity without benefit |
| Interactive onboarding wizard | Heavier UX, delays actual work | Over-engineering |

## Key decisions

**Quick experiment is primary, wave creation secondary.** This follows the improviser direction: "exploration should allow a one-off action without committing to a persistent wave." The sidebar preview teaches the mental model without requiring action.

**Steps run on entire repo by default.** For quick experiments, we don't ask for area. The whole codebase is context. If the step needs focus, the user can create a wave.

**No persistence for quick runs.** Runs appear transiently (in the "Recent Activity" section while running) but don't create permanent wave records. This keeps the sidebar clean.

## Scope

- In scope: Empty state redesign, quick step buttons, sidebar preview
- Out of scope: Auto-expiring waves, ephemeral wave type, changes to wave creation flow

## Done when

```bash
# User sees empty state
# Clicks "design" button
# Terminal opens with `lf design` running on repo root
# No wave appears in sidebar
```

A user can run a step in one click. The sidebar preview shows what waves look like without requiring one to be created.

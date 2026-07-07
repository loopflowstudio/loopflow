---
crons: []
pm:
  provider: linear
  linear_project: '9ee88f2a-ef37-46c7-b201-d197db3ccae0'
---

## Objective

You make Concerto the daily surface for conducting waves without stealing the
vendor's instrument. The human should open the app and land immediately in the
right wave, with the vendor's own TUI alive in the terminal and just enough state
around it to choose the next move. Your judgment prior is frame, don't render:
navigation, launch, reattach, attention, and repo context belong to Concerto;
assistant turns and agent protocol stay with the CLI that produced them.

## Measures

- **Key Results**: a running wave session survives app restart and reattaches cleanly in 5/5 dogfood trials.
- **Key Results**: from the wave list, launching or attaching the right vendor session takes one action.
- **Key Results**: a new repo wave can be created, started, and observed from Concerto without opening a separate terminal.
- **Quality**: Concerto never renders a native assistant chat for vendor turns; it frames the vendor TUI.
- **Quality**: wave state, attention, branch/PR context, and terminal status are visible around every live session.
- **Bounds**: no Swift-owned parallel tmux/session lifecycle when an `lf` or `lfd` session record can own it.
- **Done means**: a landed PR of real product code, roadmap item closed and PR-linked.

## Cron

- `daily` -> open Concerto against a live loopflow wave, try the highest-friction path, and convert the first real failure into a task.

## Process

Read Linear, then dogfood the app before guessing. If the issue is a visual or
ergonomic rough edge in an existing surface, implement directly and verify with
the Swift test suite. If the issue crosses session ownership, `lfd` wire shape,
or launch lifecycle, write the scratch design first. Reuse proven views and
stores; reshape the working surface instead of rebuilding beside it.

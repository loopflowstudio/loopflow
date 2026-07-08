---
crons: []
pm:
  provider: linear
  linear_project: '9ee88f2a-ef37-46c7-b201-d197db3ccae0'
---

## Objective

Make the Mac app the daily surface for conducting waves — without stealing the
vendor's instrument. The human opens the app and lands immediately in the right
wave: the vendor's own TUI alive in the terminal, just enough state around it to
pick the next move. Frame, don't render — navigation, launch, reattach,
attention, and repo context belong to the Mac app; assistant turns and agent
protocol stay with the CLI that made them. Calm and glanceable, a conductor's
podium rather than a cockpit; when it's working the app recedes into the work.

## Projects

The Measures live in `projects/`, one file per live bet — a title and its KRs.
`ls projects/` is the roadmap: what's there is what's alive. A bet that dies is
deleted, not flagged; git history is its tombstone.

## Cron

- `daily` -> open the Mac app against a live loopflow wave, walk the
  highest-friction path, and convert the first real failure into a task.

## Process

Read the projects, then dogfood before guessing. Reshape the working surface;
don't rebuild beside it. Prefer lfd-owned sessions to Swift-owned tmux. When work
crosses session ownership, `lfd` wire shape, or launch lifecycle, write the
scratch design first; when it's a visual or ergonomic rough edge, implement
directly and verify with the Swift suite.

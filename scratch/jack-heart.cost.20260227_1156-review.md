# Review: cost wave housekeeping

## What was done

Two wave management actions:

1. **Ingested inline views design** — moved `wave/cost/04-inline-views.md` out of the backlog (deleted from wave/). This item is now the active work target.
2. **Closed the tmux wave** — deleted `wave/tmux/` entirely (README, 05-polish.md, tmux.yaml). The tmux plugin shipped in prior releases; no remaining backlog items warranted keeping the wave open.

Updated the cost wave README status paragraph to describe current state clearly — what's shipped, what's next — instead of listing phase numbers.

## Key choices

- **Full deletion of tmux wave** rather than marking it "done." No future items planned, and the wave structure would just be noise. Git preserves the history.
- **README rewrite as prose** instead of phase-numbered status. The old phrasing ("Phase 01 and Phase 02 shipped") was internal shorthand that wouldn't help a reviewer or future reader understand the actual state.

## Risks

None. This is pure wave state management — no code, no tests, no user-facing behavior changed.

## What's not included

The ingested inline views design itself (the actual implementation). That's the next step in the cost wave flow.

# Questions

## Assumptions taken in this implementation

- Added first-class `run.*` / `step.*` daemon events instead of forcing journal imports through the existing `wave_*` / `agent_*` vocabulary.
- Kept the journal root fixed at `<worktree>/.lf/runtime/runs/<run_id>/...` for v1. There is no alternate root override yet.
- Journal emission currently wraps normal `lf` CLI invocations in wave worktrees. `lfd` observes them by polling known wave repos for sibling worktrees and replaying new journal lines into the event hub.

# Session registry — active agents by worktree

**Finish line:** Every `lf` session self-registers in `lfdb` on start, and
`active_sessions_by_worktree` is a real query. Concerto (and `lf d sessions`) can
enumerate live agents grouped by worktree without lfd having launched them — so
the sessions `lf goal --tmux` spawns are finally visible to live status.

## Why

This closes the concrete gap called out in
[[concerto]]'s `1-embedded-terminal-build-driver`: `lf goal --tmux` launches a
tmux session that lfd can't see, so lfd-backed status badges are blind to
client-launched work. The registry makes `lf` the run registry and the db the
source of truth — lfd just watches and pushes.

## Model

- **Group key = worktree name** (dir basename, e.g. `loopflow.goals`).
- Multiple agents under one worktree → differentiate within by session-id/step
  suffix; group by the shared worktree prefix.
- `lfdb` exposes `register_session` / `active_sessions_by_worktree`.
- `lf goal <wave>` runs **inside the wave's worktree** (`loopflow.<wave>`) — which
  already ships — and registers under that worktree name. "Show active agents"
  enumerates live sessions grouped by worktree.

## Done when

- `lf goal --tmux` (and other `lf` launches) write a session row on start.
- `active_sessions_by_worktree` returns live sessions, grouped, for a repo.
- Concerto's live status can reflect an `lf`-launched session it did not itself
  spawn.

## Depends on

[[1-lfdb-extraction]] — the registry API lives in `lfdb`.

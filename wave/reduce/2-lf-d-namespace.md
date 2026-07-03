# `lf d` — absorb lfd's reads and exec-behavior

**Finish line:** `lf d <verb>` does lfd's store reads/writes and exec-behavior
directly — waves, runs, terminal-sessions: list/get/create/launch — as
local/ssh CLI calls against `lfdb`. Concerto reads and acts through `lf d` for
everything except push subscriptions.

## Scope

- **Query routes → `lf d <verb>`** reading `lfdb` directly (no HTTP round-trip
  for request/response work).
- **Executor → `lf` / `lf d` exec paths** — tmux/docker launch, triggers,
  janitor. The proven launch mechanism already lives in
  `rust/loopflow/src/lfd/executor/wave/mod.rs` (`launch_tmux_session`); `lf goal
  --tmux` already mirrors it. This item generalizes that from the goal loop to
  the rest of the executor surface.
- Access model is shell/binary/ssh only — there is no remote-behavior-without-a-
  shell path. Push is the sole exception, and it stays in `lfd serve`
  ([[3-shrink-lfd-subscription-server]]).

## Done when

- `lf d` lists/gets/creates waves, runs, and terminal-sessions against `lfdb`.
- `lf d` can launch and dispatch exec-behavior that previously required the lfd
  HTTP executor.
- Concerto's request/response calls go through `lf d`, not lfd HTTP.

## Depends on

[[1-lfdb-extraction]].

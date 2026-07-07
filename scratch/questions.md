# Open assumptions

- `lf task <linear-item-id>` is the public command. The three-phase flow is named
  `task-pass` so it does not collide with the top-level `task` subcommand.
- The v1a runner uses the existing wave registry for worker placement. A wave
  must be registered before `lf task` can create its worker worktree.
- Run 2 first draft implements the shared pass/oracle runtime, task refit,
  project KR driver, Linear label reads, and the project/wave pass skills. The
  live wave scheduler still runs through `wave/mind.rs`; replacing that resident
  turn with `wave-pass` and doing the full mind-to-flowloop wire rename remains
  the next slice because it crosses the listener, supervisor, DTO fixtures, and
  Swift mirrors.

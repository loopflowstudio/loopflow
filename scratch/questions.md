# Questions

- LOO-1113 assumes a failed first fetch should be retried by the next
  `lf task list` (≤ 5 s each) rather than skipped while the carrier records an
  error. If an unreachable remote makes lists slow in a never-fetched clone,
  gate the read on `error IS NULL` instead.
- `before_first_read` reads Git only. A hub-only clone registered without a
  save (`lf wt create`, a launch) lists without a first hub read unless the
  server answered a command there. Not in LOO-1113.

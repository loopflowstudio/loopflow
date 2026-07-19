# Slice 2 follow-ups

- `handback_state` remains only for the existing opaque interactive Invocation
  surface. Slice 3 must replace it with Demo's explicit interactive handback.
- This slice exposes explicit `lf work asks` / `lf work answer` servicing.
  Detached Project and Wave answer agents remain Slice 3 work.
- After runner loss, `lf ask wait <ask-id>` may recover an exchange from an
  earlier Invocation in the same Work Epoch. The explicit id is the fence that
  distinguishes it from a new Turn's own Ask.
- Linear's existing comment mutation targets issue ids, so the provider-write
  outbox mirrors Task Ask/Answer exchanges. Project and Wave Work have no
  compatible Linear comment target in this slice.
- `scripts/check_migrations.py` is currently fenced before this change by the
  branch's `0.11.037_after_merge_continue_task` colliding with
  `0.11.037_capture_terminal_states` on `origin/main`; integration must rebase
  and reallocate that existing migration.

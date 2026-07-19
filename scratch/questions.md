# Slice 2 follow-ups

- `handback_state` remains only for the existing opaque interactive Invocation
  surface. Slice 3 must replace it with Demo's explicit interactive handback.
- This slice exposes explicit `lf work asks` / `lf work answer` servicing.
  Detached Project and Wave answer agents remain Slice 3 work.
- After runner loss, `lf ask wait <ask-id>` may recover an exchange from an
  earlier Invocation in the same Work Epoch. The explicit id is the fence that
  distinguishes it from a new Turn's own Ask.

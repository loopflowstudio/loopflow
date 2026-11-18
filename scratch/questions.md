# Open choices (LOO-977)

- The plan also types Codex usage limits from `codexErrorInfo:
  "usageLimitExceeded"`, beyond the brief's apostrophe match. Drop slice 2 if
  only the wording fix is wanted.
- Unconfirmed: whether the 2026-11-18 failure carried `usageLimitExceeded`.
  No raw payload was kept. If a spent Codex account is at hand, one failed
  turn's `error` notification would settle it.
- Later: map `rateLimitExceeded`, `serverOverloaded`, `modelNotFound` from
  `codexErrorInfo` to typed failures and retire the matching text phrases for
  Codex.

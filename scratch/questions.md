# Open choices (LOO-983)

- **Commit messages stop moving on non-limit failures.** The loop in
  `ops/commit.rs` tried the next agent after any failure (timeout, bad reply).
  Folded into the shared loop, it moves on a usage limit, unavailable model or
  revoked login only. Assumed fine because transient failures retry in place
  and a step commit falls back to `lf <step>: checkpoint`. If moving on a
  timeout or an unparseable reply matters, that belongs in the shared loop
  for every caller, not in a second loop.

- **`codexErrorInfo` on a live usage refusal is unobserved.** The 0.170.0
  schema lists `usageLimitExceeded`, but no captured trace of a real refusal
  exists. The apostrophe fold is what the logs prove; the structured read is
  added beside it. A trace captured the next time Codex refuses would settle
  whether the text match can go.

- **`lf replay` on a bare harness now tries that harness's other models on a
  limit**, like every pinned bare launch. Left as is.

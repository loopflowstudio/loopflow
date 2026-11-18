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

- **`lf replay` runs the recorded agent and nothing else.** The bare-harness
  model expansion lives only in `Config::resolve_agents`; replay builds its
  launch by hand, so it never moves. Same as today.

- **A Codex-to-Codex move resumes a thread whose first turn was refused.** It
  sends `LIMIT_FAILOVER_PROMPT` to that thread. No Codex refusal has ever
  classified, so this path has never run against the provider; whether the
  thread still holds the task is unknown. If it does not, a move within a
  provider with no reply yet should also restart from the original prompt.

- **A turn refused before any reply restarts from the original prompt on the
  next provider**, instead of importing the transcript. Chosen because a PR
  prompt's diff is longer than the import's excerpt. Passed over: a flag on
  the launch marking it one-shot.

- **A provider move rewrites the Session's `provider` and `model`.** Needed so
  the Session closes and reads the transcript of the agent it now runs. A
  model move inside one provider leaves `model` as it was, same as before.
  Whether Session lists should show the first agent or the current one was
  not asked; they now show the current one.

- **Stderr prefix on a Codex refusal read from `codexErrorInfo`** is
  `provider_rate_limited: You’ve hit…`, where the docs' example shows
  `codex_error: You’ve hit…` (what the text match yields). Which one people
  see depends on the unobserved field.

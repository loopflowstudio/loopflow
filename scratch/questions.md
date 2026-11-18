# Open choices (LOO-983)

- **Commit messages stop moving on non-limit failures.** The loop in
  `ops/commit.rs` tried the next agent after any failure (timeout, bad reply).
  Folded into the shared loop, it moves on a usage limit, unavailable model or
  revoked login only. Assumed fine because transient failures retry in place
  and a step commit falls back to `lf <step>: checkpoint`. If moving on a
  timeout or an unparseable reply matters, that belongs in the shared loop
  for every caller, not in a second loop.

- **`codexErrorInfo` on a live usage refusal is `usageLimitExceeded`.**
  Observed 2026-11-18 against the account's Codex (codex-cli 0.170.0, refused
  until 3:36 PM): every move classified from the field, and stderr read
  `provider_rate_limited: You’ve hit your usage limit. …`. The text match is
  now the second reader; whether it can go was not decided.

- **`lf replay` runs the recorded agent and nothing else.** The bare-harness
  model expansion lives only in `Config::resolve_agents`; replay builds its
  launch by hand, so it never moves. Same as today.

- **A Codex-to-Codex move resumes the refused thread.** Ran live 2026-11-18:
  `codex:gpt-6.2-sol → codex:gpt-6.2-astra` resumed (`resumed=true` in
  `lf.log`) and the second model was refused too, since the limit is
  account-wide. Whether the resumed thread still holds the task when the
  second model answers is still unobserved.

- **A turn refused before any reply restarts from the original prompt on the
  next provider**, instead of importing the transcript. Chosen because a PR
  prompt's diff is longer than the import's excerpt. Passed over: a flag on
  the launch marking it one-shot.

- **A provider move rewrites the Session's `provider` and `model`.** Needed so
  the Session closes and reads the transcript of the agent it now runs. A
  model move inside one provider leaves `model` as it was, same as before.
  Whether Session lists should show the first agent or the current one was
  not asked; they now show the current one.

- **An input answered after a move names its provider through the Session.**
  The capture row keeps the first agent and cannot change, so
  `input_answered_turn` takes the Session's provider when the answering thread
  is the Session's current thread. Found by `lf -b research`: its second step
  failed with "the transcript … is gone: ~/.codex has no file for it". Left
  open: a Session that moved inside one input and later switched provider again
  reads that old input's answer from history's copy only. A `turn_provider`
  column beside `turn_thread` would close it; passed over as a schema change.

- **A batch Graph's steps wait.** `lf -b research '<question>'` is a Graph, so
  with the whole order refused it waits 5, 15, 30, 60 minutes at the terminal
  where `lf -b skill -- unbreak` stops at once. Before this change a Codex-led
  one failed at once, because the refusal never classified. The docs now say
  so; whether a Graph run in the foreground should wait was not decided.

- **`lf commit` with its order spent still commits as `lf commit`.** It now
  prints why the message failed; before, it printed nothing. Whether it should
  stop instead was not decided.

- **A headless turn sent to an existing conversation does not move.**
  `lf -b session resume <id> --message …` on a refused Codex conversation stops
  with "No fallback remains … If `agents` is missing or has only one entry",
  though `agents` has four. `-a claude:haiku` moves it by hand. Left as is: a
  conversation keeps its provider; the advice does not fit this case.

- **`-a claude` with every Claude model refused** ends "add another agent to
  `agents`", which the pin ignores. A launch cannot tell a pin from an
  unconfigured default, where that advice is right.

- **Sync onto main (2026-11-18) took main's `record_session_agent` and
  `codex_decode::error_code`.** Main had landed both independently. Its
  version writes `provider`/`model` on every retry, so a model move inside one
  provider now updates `model` too (the note above said it stayed). Its
  `error_code` also maps `modelNotFound`, `unauthorized` and
  `serverOverloaded`, so the branch's text-only classification of those two is
  now a second reader. The branch's `a_spent_quota_is_a_usage_limit_wherever_codex_reports_it`
  repeated main's `each_stated_cause_has_its_code_on_the_error_and_on_the_failed_turn`
  and is deleted.

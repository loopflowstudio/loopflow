# Questions

- Open, cannot be undone: when is the Discord bot's token revoked? The concept
  review recommends before the merge (revoke in Discord, delete the secret
  from Doppler `loopflow/dev`, remove the bot from the guild), so the PR lists
  them as done. The plan does not hold the merge for it. Until it is revoked,
  an agent started under `doppler run -c dev -- lf …` can read it, as it can
  every other secret in that config except `GH_TOKEN` and `OPENCODE_API_KEY`.
- Assumed (made for review): `Surface` went to two variants in this change,
  and the pass after deleted it for `harness::Mode`, with `lf-prompt --mode
  headless|interactive` rejecting any other word. The steer named only
  `Surface::Chat`. Passed over: keeping "surface" as the prompt's own word
  for a future phone or unwatched-terminal prompt.
- Assumed (made for review): a leftover Wave `chat:` block is ignored
  silently, in a list of retired Wave keys beside `linear` and apart from
  `id`, with a removal Task filed in this change.
- Assumed: one Task, LOO-1121, deletes both retired Wave keys. No existing
  Task named the `linear` key (`lf task list linear`), so it joins `chat`.
- Assumed: the golden prompt cases carry no `mode` key. All six said
  `headless`, so the test renders headless and nothing parses the word. An
  interactive golden would bring the key back.
- Assumed: `Mode::default()` is headless, as `Surface`'s was, so
  `GatherContextOpts` and `PromptComponents` keep `Default`.
- Not checked: whether `builtin_skills_use_the_application_fallback` fails at
  the parent commit on this Machine; it reads installed harnesses and logins.
- Not checked: whether `lf discord serve` runs on a Machine other than this
  one and 100.96.227.95.
- Assumed at ship review, with nobody in the conversation: the work goes on
  to delivery. No Task was filed. The token steps stay in the PR body; the
  recorded `surface` string, the doubled Wave warning and the misspelt
  `deafult_agent` key in `~/.lf/config.yaml` are on
  `scratch/ship-review.html` as questions 1 to 4, unanswered.

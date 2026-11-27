# Questions

- Open, cannot be undone: when is the Discord bot's token revoked? The concept
  review recommends before the merge (revoke in Discord, delete the secret
  from Doppler `loopflow/dev`, remove the bot from the guild), so the PR lists
  them as done. The plan does not hold the merge for it. Until it is revoked,
  an agent started under `doppler run -c dev -- lf …` can read it, as it can
  every other secret in that config except `GH_TOKEN` and `OPENCODE_API_KEY`.
- Assumed (made for review): `Surface` goes to two variants, `HumanPresent`
  and `Headless`, in this change. `lf-prompt --surface cli|mac|iphone|ide`
  then read as headless. The steer named only `Surface::Chat`.
- Assumed (made for review): a leftover Wave `chat:` block is ignored
  silently, in a list of retired Wave keys beside `linear` and apart from
  `id`, with a removal Task filed in this change.
- Assumed: one Task, LOO-1121, deletes both retired Wave keys. No existing
  Task named the `linear` key (`lf task list linear`), so it joins `chat`.
- Assumed: the golden prompt cases parse `surface` with `FromStr` so the
  serde derives could go; keeping the derive would spell the variant
  `human_present` in YAML and `human-present` on the command line.
- Not checked: whether `lf discord serve` runs on a Machine other than this
  one and 100.96.227.95.

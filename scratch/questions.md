# Questions

- Assumed: a leftover Wave `chat:` block is ignored silently (`"chat"` stays in
  `WAVE_ONLY_KEYS` beside `linear`, with a removal Task) instead of warning
  `skipping chat: unknown setting` in every checkout that predates the merge.
- Assumed: lf stops stripping `LF_DISCORD_TOKEN` from agent launches now that
  nothing reads it. If the token stays live in Doppler, a `doppler run -- lf …`
  launch hands it to the agent. Revoking the token closes this.
- Not checked: whether `lf discord serve` is running on another Machine
  (mini-heart). A running bridge keeps answering until its process is stopped.

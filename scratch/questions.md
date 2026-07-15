# W2-151 open items

## Blocker (external, transient)

PR 1/3 is committed, rebased onto origin/main, pushed to
`jack-heart/make-every-cli-command-resolve`, and fully green. `lf pr open`
could not create the PR object: `gh` hit `GraphQL: API rate limit already
exceeded for user ID 37011`. This is an external GitHub limit, not an auth or
code problem — retry `lf pr open` (or `lf pr land --next chat-radio-memory`)
once the limit resets (~1h).

## Serial PRs remaining (see scratch/make-every-cli-command-resolve.md)

- **PR 2 — chat / radio / memory**: route their `LF_WAVE_ID` (`WaveId`) arm
  through `resolve_managed_wave_name`; add the name-fallback + classified errors
  they currently lack. `LF_CHANNEL` arm untouched.
- **PR 3 — trace + home + cron launch helpers**: `resolve_run_wave_name` /
  `wave_name_for_id` gain the hand-set-name fallback; `lf cron add`
  (`ops/mod.rs:895`) resolves ambient like the PM arms.

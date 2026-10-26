# LOO-601 questions and assumptions

- Nested hidden commands/flags: Jack Heart asked for clear wins only; the plan lists
  what became visible and what waits for LOO-610.
- `telemetry-scorecard` is an agent skill, so the daily Graph now spends one headless
  agent turn. That follows Jack's "simple skill" decision. A `cmd:` step can only run
  lf commands, so a non-agent step would mean bringing back an lf command.
- `--cwd` accepts relative paths (resolved against the directory reached after
  `--repo`/`--repository`); the old hidden flag required an absolute path.
- `__repair`: a failed run (no conclusion) blocks the landing once before the next
  check retries it, matching the old worker's admit-error behaviour.
- A ci-fix run is now an ordinary `lf skill` run, so like every skill run it adds
  `.lf/prompts/` to a repository `.gitignore` that lacks it. The release repair fixture
  and land repair fixtures commit that entry, as real repositories do; a repo without it would see the
  repair checkout dirtied, and `lf arm` refuse, on its first repair.
- In a Task checkout the ci-fix run is now confined like every unattended Task skill
  step, so it needs a connected managed provider account; the old hidden worker ran on
  ambient credentials. Without an account the run fails, the landing blocks once with
  "no eligible managed … account", and the next check retries. The land fixtures now
  connect a managed codex account (shared `support::managed_codex_account`).

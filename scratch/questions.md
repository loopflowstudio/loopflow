# LOO-957 open questions (kickoff, 2026-11-17)

Choices made headless for review; the plan says what each passed over.

- Claude signs Machines in with `claude auth login` on the target, not
  `claude setup-token`: the probe found no paste-code path for `setup-token`
  without a TTY. This also takes away lf storing any Claude token. Passed over:
  setup-token minted here and stored in the target's lf store
  (`provider_tokens` -> `CLAUDE_CODE_OAUTH_TOKEN` per launch).
- Signing in this Machine also uses the paste-code URL, so here and there share
  one path. Passed over: `$BROWSER` set to an lf shim so Claude's localhost
  callback completes without pasting.
- A rental's `logins` list comes from its on-demand template. Passed over: a
  rent-time flag persisted in a `machines.logins` store column.
- The launch login check runs on every launch, local too, fail-open, bounded at
  0.5 s p95. Passed over: only behind SSH.
- `lf machine disconnect` is kept (runs each provider's own logout).
- An unasserted existing login on a target: `lf machine connect` replaces it
  (the provider's login overwrites its own file); rental prepare and
  `--refused` skip a provider that is already signed in and not refused.
- Not tested: whether device code login works on the ChatGPT plan in use, and
  whether either approval page tolerates an automated browser.

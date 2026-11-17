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
- Concept review: `logins` lives only on `ondemands.<name>`; owned Machines
  have no list, and `own_logins` is gone (not running the command is the
  opt-out). Passed over: lists on machines/pools/on-demands plus `own_logins`.
- Concept review: the verb is `lf machine login <provider> [<machine>…]`,
  `logout`, `logins`; `connect` already means the SSH destination in
  `lf machine`. Passed over: `connect`/`disconnect` with machine first.
- Concept review: `--refused` is derived (probe signed out, or refused with no
  successful launch since); no sign-in time is recorded. Open: whether a
  usage-limit refusal should count, since signing in again helps only if the
  approving browser now holds a different account.
- The launch login check runs on every launch, local too, fail-open, bounded at
  0.5 s p95. Passed over: only behind SSH.
- `lf machine logout` is kept (runs each provider's own logout).
- An existing login on a target: `lf machine login` replaces it (the
  provider's login overwrites its own file); rental prepare and `--refused`
  skip a provider that is already signed in and not refused.
- Not tested: whether device code login works on the ChatGPT plan in use, and
  whether either approval page tolerates an automated browser.
- Docs: `logins` is a `--logins` flag on `lf ondemand add` stored in an
  `ondemands` column, since templates live in the store, not config. Passed
  over: a `logins` config key beside the store template.
- Docs: `LF_APPROVE_CODE` is passed whenever the page asks for a device code
  (Codex, GitHub, OpenCode). Passed over: Codex only.

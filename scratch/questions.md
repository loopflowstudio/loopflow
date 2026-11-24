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
- Docs: `LF_APPROVE_CODE` is passed whenever the page asks for a device code
  (Codex, GitHub, OpenCode). Passed over: Codex only.
- Slice 1: the pasted Claude code echoes in the terminal (no raw-mode read).
  It is a one-time PKCE code useless without the target's verifier, and a
  no-echo read leaves the terminal silent after Ctrl-C. Passed over: termios
  echo off with a restore guard.
- Slice 1: `lf machine login <provider> a b` tries every Machine and fails at
  the end naming those not signed in. Passed over: stop at the first failure.
- Slice 2: `lf machine logins --json` is a bare array of
  `{provider, state: signed_in|signed_out|unknown, account}`; Rust is its only
  reader, so no DTO fixture under `tests/fixtures/dto/`. Passed over: an object
  wrapping the rows with the Machine's name.
- Slice 2: a missing client reads `unknown`, not `signed out`, as the plan's
  probe rule says; a launch then fails on its own.
- Slice 2: an approver command runs to completion before lf waits on the
  login; a command that never exits is not cut off at the 15-minute deadline.
- Slice 2: the `auth_browser_bindings` export in `config/migrate.rs` is
  deleted with the setting; an old store's bindings are no longer copied to
  config, since nothing reads them.
- Slice 3: `--logins` is stored in the `ondemands.<name>` config template
  (`OnDemandDeclaration.logins`), written only when set, because templates live
  in `~/.lf/config.yaml` and `OnDemandDeclaration` denies unknown fields: a
  released lf reading a template with `logins` fails to load it. Passed over:
  an `ondemands` store column, which the docs named but no template row holds.
- Slice 3: the launch probe runs with the launch's env, so a per-run
  `CLAUDE_CODE_OAUTH_TOKEN` or API key counts only if the client's own status
  command sees it. Not checked against a real token.
- Slice 3: the launch probe deadline is 2 s; a slower client launches anyway.
- Slice 3: `lf ondemand rent` (by hand) signs nothing in; its hint names
  `lf machine login <provider> <label>`. Only `--rent` placement prepares.
- Slice 4: `--refused` counts any `agent_limit_recoveries` row as a refusal
  (usage limit, revoked or missing login, model unavailable, a stop) and any
  later agent process of that provider as the answer, started, not
  necessarily successful. A refused Machine stays selected until a launch
  there, so two `--refused` runs sign it in twice. Passed over: recording a
  sign-in time.
- Slice 4: `--refused` covers every added Machine, rentals included; one that
  cannot answer is reported and fails the run.
- Slice 4: `logins` reads GitHub from gh's `hosts.yml` `user` wherever gh keeps
  the token, so the transfer pre-check skips a keyring-backed gh for the same
  user that it used to replace with a file-backed token. OpenCode's row is the
  active export entry with no Zen check, so a same-account entry Zen rejects
  is skipped too. Linear's pre-check compares the target's stored login with
  the viewer id; a mismatch renews and sends, and the receiving end decides.
- Slice 4: `lf account …` now reads as a skill name ("skill or graph not
  found: account"); no moved-command hint names `lf machine login`.
- QA: `--refused` covers added Machines only, so this Machine reading Codex
  refused is not selected; `lf machine login codex` signs it in. Passed over:
  including this Machine.
- QA: after a code, a Claude login still running 1 s after its first stderr line ends as rejected
  (a malformed code makes it complain and wait for another paste). Passed
  over: reading another code from the person.
- Concept review (as built): Asana, Notion and Doppler sign in on this
  Machine only; one predicate (`login::reaches_other_machines`) gates
  dispatch, rental prepare's default and `ondemand add --logins`. Passed over:
  remote transfers for them.
- `lf machine login asana <any Machine>` and `--refused` are refused before
  resolution, even naming this Machine's label, though `login::login` would
  sign this Machine in; naming no Machine is the way. One check at the command
  (`login::require_reach`), none inside `login::login`. Passed over:
  resolving first, which reaches SSH.
- Rescope docs: `lf machine logins mini` overlays Claude as `forwarded`
  from this Machine's `forward.env`, since the target's probe runs without the
  per-run token and would read `signed out` for a Machine whose launches work.
  Passed over: no overlay, with a sentence explaining the `signed out` row.
- Rescope docs: Claude drops out of `approvers` entirely; local
  `lf machine login claude` hands the terminal to `claude auth login`. Passed
  over: keeping the paste-code relay for this Machine only.
- Rescope implement: `lf machine login claude` here runs plain
  `claude auth login` on the inherited terminal, ignoring `approvers.claude`
  and `--chrome-profile`, which a leftover setting now does nothing for.
  Passed over: rejecting an `approvers.claude` entry in config validation.
- Rescope implement: the `forwarded` overlay replaces the target's own Claude
  row even when that row reads signed in (a forwarded token is what each
  launch uses). Passed over: overlaying only a signed-out row.
- Rescope implement: `lf machine logout claude mini` still runs
  `claude auth logout` there. Passed over: refusing it like login.
- Sync onto main (2026-11-23): main replaced `forwarding_environment` with
  `wire::Selection`, which has no target, so `LF_MACHINE_NAME` now rides in
  the launch's `Secrets` (`Secrets::carry_launcher_name`) for server requests
  and beside `LF_EXPECTED_MACHINE_ID` for `lf ssh`. Passed over: a
  `Selection` field, which changes the wire schema and every `capture` caller.
- Sync onto main: main dropped Notion; it is gone from logins, docs and tests.
  Revoked-login advice is `lf machine login <provider>` for a provider lf signs
  in, else the harness's own (`agy`, Pi).
- Concept review (2026-11-23, as built after rescope): a `forward.env`
  `CLAUDE_CODE_OAUTH_TOKEN` with no value here is left out of the forward
  instead of stopping every launch; Claude then reads signed out on the target
  and the launch check falls back. Passed over: keeping main's stop for it, a
  new `LoginState`, forwarding without `forward.env`. Open (page question 3):
  whether `lf machine logout claude <machine>` should refuse like login.
- Concept review (2026-11-23, at 7a0119e430): `lf machine logout claude
  <machine>` stays: it signs out that Machine's own Claude login, as logout
  does per Machine for every provider (Asana and Doppler run `lf machine
  logout` there). Passed over: refusing it like login, which would leave no lf
  way to clear a hand-made login there.

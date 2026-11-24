# LOO-957: Sign Machines in the way each provider documents

## Rescope (2026-11-23)

Observed 2026-11-23: modal-3…6 (Claude Code 2.1.311) read `loggedIn: false`
for Claude; their steps ran on Codex. `main` carries Claude to a rental only
through `forward.env`, and nothing named `CLAUDE_CODE_OAUTH_TOKEN`. The Doppler
`CLAUDE_CODE_OAUTH_TOKEN` (a `claude setup-token`) answered from an empty HOME
(`authMethod: oauth_token`). `forward.env: [CLAUDE_CODE_OAUTH_TOKEN]` is now
set in personal config.

Asked "should LOO-957 shrink to: Claude by forwarded token; Codex keeps the
sign-in on the rental?" Answer: "ok lets switch to that", then "go ahead and
rebuil".

So:
- **Claude reaches other Machines only as a forwarded setup token**
  (`forward.env` naming `CLAUDE_CODE_OAUTH_TOKEN`). Delete the remote
  `claude auth login` paste-code path, Claude's approver and its tests.
  `lf machine login claude` with no Machine still runs Claude's own login
  here; naming another Machine refuses and says to forward
  `CLAUDE_CODE_OAUTH_TOKEN` (after `claude setup-token` and
  `doppler secrets set CLAUDE_CODE_OAUTH_TOKEN`).
- **Codex keeps the device login on the target** (`codex login --device-auth`),
  approvers and `--refused` included.
- **Rental prepare** stops signing Claude in; it relies on `forward`, and
  `main`'s check that a rental carries some agent login stays.
- **The launch login check** counts Claude signed in when the launch carries
  `CLAUDE_CODE_OAUTH_TOKEN` (the probe already runs with the launch's env;
  verify it does for a forwarded variable), and its refusal for Claude names
  `forward.env`, not `lf machine login claude <machine>`.
- `lf account` retirement, services, and Asana/Doppler staying here (main dropped Notion)
  are unchanged.
- Docs (`docs/subscriptions.md`, `docs/ops.md`, help text) say the same.

Docs for the rescope (written first, 2026-11-23): `docs/subscriptions.md`
("Sign in on any Machine", "Choose who approves", "Rentals", "Per-run
secrets"), `docs/ops.md`, `docs/security.md`, `docs/dev/machines.md`,
`docs/config.md` ("Sign-in approvers"), `README.md`. Settled there, for
implement:
- `approvers` no longer covers Claude; `lf machine login claude` here runs
  `claude auth login` in the terminal with its own page (no paste relay).
- `lf machine login claude <machine>` refuses with one message naming
  `claude setup-token`, `doppler secrets set CLAUDE_CODE_OAUTH_TOKEN` and
  `forward.env`; `claude` leaves `login::reaches_other_machines`, so
  `--logins claude` and `--refused` for Claude are rejected too.
- Launch refusal for Claude: `mini has no Claude login; forward
  CLAUDE_CODE_OAUTH_TOKEN (`lf set --global forward.env`)`.
- `lf machine logins <machine>` shows Claude as `forwarded
  CLAUDE_CODE_OAUTH_TOKEN` when this Machine's `forward.env` names it (the
  sending side overlays the row; the target's own probe can't see a per-run
  token). New `state: forwarded` in the `--json` rows.
- Rentals: prepare signs Claude in to nothing; the rental stops before renting
  when it has neither a forwarded token nor Codex to sign in (main's check).

**Built (2026-11-23).** `login::reaches_other_machines` drops Claude;
`login::require_reach(provider, machine)` refuses Claude with the forwarded-token
message (CLI `login`/`--refused`, `ondemand add --logins`). `lf machine login
claude` here runs `claude auth login` on this terminal, then confirms with the
probe; the paste-code path, `REJECTION_GRACE`, `Login::answer` and the approver's
returned code are gone (`Sent::Approved` carries nothing). `LoginState::Forwarded`
overlays the Claude row of a remote `logins` when `forward.env` names
`login::CLAUDE_TOKEN_ENV`. `login::sign_in_advice(Provider)` replaces `sign_in_command`
and the Claude and Codex harnesses' own advice; `login::missing_login` is the
launch check and its refusal line. Rental `place` computes the sign-in
list (`rental::sign_ins`) before renting and stops when it holds no Codex and
`forward` carries no `CLAUDE_CODE_OAUTH_TOKEN` (`Forwarded::carries_env`).
Verified: the forwarded variable reaches the remote launch probe
(`session_launch_contract::a_signed_out_machine_refuses_the_launch_with_the_command_that_signs_it_in`,
now with a Claude case). Fixtures: `machine_commands::a_command_approver_signs_machines_in_to_codex_without_a_terminal`
(the `forwarded` row), `services_that_sign_in_here_only_never_reach_another_machine`
(Claude's refusal, no SSH). Not tested: the rental no-agent-login stop (no
rental fixture reaches `place`'s rent path); local `lf machine login claude`
(hands the terminal to the real client).

## Outcome

`lf machine login <provider> [<machine>…]` is the one sign-in command, for this
Machine (no Machine named) and any other. Codex runs its own device login
**on the target Machine** and sends the approval page to this Machine's
browser; the credential is created where it is used and never crosses the
network. Claude reaches other Machines only as a forwarded
`CLAUDE_CODE_OAUTH_TOKEN` (`claude setup-token`, kept in Doppler, named in
`forward.env`); here, `lf machine login claude` runs `claude auth login`.
`lf account` is gone; services (GitHub, Linear, Asana, Doppler, OpenCode) sign
in through the same command.

## Demo

```bash
lf set --global forward.env '[CLAUDE_CODE_OAUTH_TOKEN]'
lf set --global approvers.codex.chrome Work
lf machine login codex mini
#   Approve Codex for mini in Chrome (Work) with code ABCD-1234
#   mini: Codex signed in (ChatGPT)
lf machine logins mini
#   claude   forwarded   CLAUDE_CODE_OAUTH_TOKEN
#   codex    signed in   ChatGPT
#   github   signed in   jack-heart
#   linear   signed out
lf machine login claude mini
#   refuses: forward CLAUDE_CODE_OAUTH_TOKEN (claude setup-token, doppler secrets set …)
lf --machine mini -a codex implement      # on a mini with no Codex login:
#   mini has no Codex login; run `lf machine login codex mini`
```

## What the probes showed (2026-11-17, Claude Code 2.1.326, codex-cli 0.169.0)

These decide the mechanism; each was run with an empty `HOME` and no browser.

- `claude auth login` with stdin not a TTY prints
  `If the browser didn't open, visit: https://claude.com/cai/oauth/authorize?…redirect_uri=https://platform.claude.com/oauth/code/callback…`
  then `Paste code here if prompted >`, and reads the code from stdin. It hands
  a second, localhost-callback URL to `$BROWSER` (it honors `$BROWSER`, not
  only `open`). PKCE: the pasted code is useless without the verifier, which
  stays inside the remote client.
- `claude setup-token` without a TTY prints nothing and only offers a
  localhost-callback URL to `$BROWSER`: no paste-code path, so it cannot be
  driven over SSH without a PTY and port forwarding. **Not used.**
- `codex login --device-auth` prints `https://auth.openai.com/codex/device` and
  a one-time code (15 minutes), polls, and writes `~/.codex/auth.json` itself.
  A code was issued; whether approval succeeds on the plan in use (device code
  is beta, enabled in ChatGPT security settings) is unproven until someone
  approves one.
- `claude auth status --json` exits 1 with `loggedIn: false` when signed out;
  signed in it reports `email`, `orgName`, `subscriptionType`, `loginExpiresAt`.
  0.4–0.8 s. `codex login status` exits 1 `Not logged in` / 0 `Logged in using
  ChatGPT`, 0.1 s, no account.

## Design

### One sign-in: run the provider's flow on the target, approve here

`login::login(machine, provider, …)`; a Machine with route `local` is this one.
For Codex: start `BROWSER=true codex login --device-auth` (a local child, or
one `bounded_ssh_args` exchange with no PTY, under `TIED_TO_STDIN` so the
remote login dies when lf's stdin closes), read the URL and code, hand them to
the approver, wait for Codex's own poll, then probe (`codex login status`).
Nothing of that output is shown or logged.

For Claude here: plain `claude auth login` on the inherited terminal, then the
probe. For Claude elsewhere: `login::require_reach` refuses before any SSH and
names `claude setup-token`, `doppler secrets set CLAUDE_CODE_OAUTH_TOKEN` and
`forward.env`. lf stores no Claude token; Doppler holds it and `forward`
carries it per launch.

Services keep their transfer in `machine/credentials.rs` (GitHub token,
refreshed Linear access token, fresh OpenCode device login).

### The approver: one personal setting per provider

```yaml
approvers:
  codex:  {chrome: Work}
  github: {command: "subpersona approve"}
```

`approvers: BTreeMap<Provider, Approver>`, Personal, HostLocal, in
`config/catalog.rs` (category Accounts). `Approver` is `Chrome(String)` (a
profile name or directory, resolved by `profile::resolve_local_chrome_profile`)
or `Command(String)`.

| Approver | What lf does |
| --- | --- |
| unset | opens the default browser (`open`/`xdg-open`); a person approves |
| `chrome: <profile>` | `open_chrome_profile`; a person approves |
| `command: <cmd>` | runs `sh -c <cmd>` with `LF_APPROVE_PROVIDER`, `LF_APPROVE_URL`, `LF_APPROVE_CODE` (any device-code page), `LF_APPROVE_MACHINE`; exit 0 is approved |

The setting covers Codex and the services' OAuth and device pages (GitHub,
Linear, Asana, OpenCode). Claude is not covered: here its own client opens its
page, and a leftover `approvers.claude` does nothing. `--chrome-profile`
on `lf machine login` overrides the setting for one run. A persona switcher
rewrites `approvers.*`; an agent-driven browser is a command. lf keeps no
account and no account-to-profile table.

This replaces `auth_browsers` (`default` list plus a per-account `accounts`
map) and the `access_profiles` store rows it pointed at. The branch stops
reading `auth_browsers`; the released lf keeps reading its own key, and
unknown keys only warn, so neither breaks the other. No fallback from old to
new: setting `approvers` once is the migration. The `access_profiles` table
stays (a drop is a staged contract); only its code goes.

### Which providers a rental is signed in to

`logins: Vec<Provider>` on `ondemands.<name>` (`OnDemandDeclaration.logins`), the template that rents and
prepares a Machine; unset, every provider signed in here. Rental `prepare`
(`lf/commands/rental.rs`) already signs in GitHub and Linear; it signs in this
list instead, at most one approval per provider, none with command approvers.
It does not follow a request's agent selection.

An owned Machine has no list and no opt-out. It is signed in to what someone
ran `lf machine login` for, and `lf machine logins` reads that live. **Made for
review** (concept review, 2026-11-17): the brief's per-Machine list ("each
Machine has its own list of providers to share with it") and `own_logins`
("a Machine can assert its own logins") were written for the copy model, where
sharing ran as a standing policy. Here a sign-in only happens when asked, the
provider's client makes every Claude/Codex login the Machine's own, and the
service receive path already leaves a different installed account unchanged.
Passed over: `logins` on `machines`/`pools`/`ondemands` plus `own_logins` on the
target, which had one reader (prepare) and an opt-out that refused explicit
requests. A list on `machines.<name>` can return alone if it earns a reader.

### Every remote launch checks first

The harness gains a native-login probe (`claude auth status`,
`codex login status`) returning signed-in, signed-out or unknown. Before a
launch starts an agent of provider P, a definite signed-out is handled like a
revoked login (`agent/mod.rs`, `AgentFailure::CredentialInvalidated`): skip P's
models, continue on the next provider in the order, and with none left stop
with the sign-in advice. The target reports only "P signed out"; the side that
launched the step, which knows the name it gave the Machine, writes
`lf machine login <provider> <machine>` (`lf machine login <provider>` here),
replacing `_sign_in_advice`'s `claude` / `/login` text. Unknown (probe missing, timed out,
unparseable) launches anyway: lf never refuses a working login on its own
misreading.

It runs on every launch, local too: one rule, no "am I remote" branch. Cost
bound: under 0.5 s added to a headless launch at p95, measured at implement;
past that it moves inside the SSH launch preamble only.

### Seeing and retrying

`lf machine logins [<machine>] [--json]` replaces `lf account [<service>]`:
per provider, `signed in | signed out | unknown` (unknown: the probe failed;
shown, never refused on), with the account where the client reports one
(Claude email and plan; Codex none). Remote reads run `lf machine logins --json`
on the target over the existing `credentials::remote_command`; that is the one
remote reader, and `lf machine credentials inspect` goes. It also reports each provider's last refusal on
that Machine (login missing or usage limit, from the recorded limit
recoveries) with its time.

`lf machine login <provider> --refused` runs the sign-in on every added Machine
whose `logins --json` shows that provider signed out, or whose last launch of
it was refused with no successful launch since. Both are derived from what the
target already has (the probe, its limit-recovery records); lf records no
sign-in time.
The approving browser picks the account, so a retry needs no account switch to
be meaningful. This is the "resend token for all failed machines" command,
without a token to resend.

`lf machine logout <provider> [<machine>…]` runs the provider's own logout
(`claude auth logout`, `codex logout`, `gh auth logout`, the stored token for
Linear/Asana/Doppler/OpenCode). Kept because retiring
`lf account disconnect` would otherwise drop it.

## Surfaces

| Surface | Change |
| --- | --- |
| `lf machine login <provider> [<machine>…]` | replaces `lf machine connect <target> <provider> [<email>]`: provider first so one positional always parses, no Machine means this one, several Machines at once; Claude, Codex and every service; `--chrome-profile`, `--api-key` (OpenCode), `--refused`; the `email` positional goes |
| `lf machine logins`, `lf machine logout` | new; replace `lf account`, `lf account disconnect` |
| `lf machine credentials inspect` | deleted; the target's `lf machine logins --json` is the remote read |
| `lf machine credentials receive` | stays, the remote end of service transfers |
| `lf account …` | deleted: `Commands::Account`, `AccountCommand`, `misuse`, `parse_service_provider`, the `navigation.rs` provider-as-value case |
| Served routes `account`, `account.disconnect`, `account.list` and fixtures under `tests/fixtures/dto/server/account*.json` | deleted; `machine.logins` read route added only if a caller needs it (Desktop has none today) |
| Desktop `RegistryQuery.swift` `accountList/Connect/Disconnect` and `ServerProtocolTests` cases | deleted; no view uses them |
| `forward.env` / `forward.files` | stay for per-run secrets; `AGENT_LOGIN_ENV`, `AGENT_LOGIN_FILES`, `carries_agent_login`, `read_for_rental` go |
| Docs | `docs/subscriptions.md` "Remote Machines and rentals" and "Service authentication", `security.md`, `dev/machines.md`, `ops.md`, `work.md`, `config.md`, `dev/reference.md`, `dev/codebase.md`, `server/README.md`; `Provider::api_key_configure_error` messages |
| Builtin skills | `init.md`, `repo_operate.md` |

## Absent and error states

- Target unreachable: the SSH error, nothing started.
- Client missing on the target: "Codex is not installed on mini; …".
- Approver command exits non-zero, or the login exits non-zero: the login's
  last error line (never its URL or code), nothing written.
- Codex device code disabled for the account: Codex's own error, plus "enable
  device code login in ChatGPT security settings".
- Approval not finished in 15 minutes (Codex's code lifetime): kill the
  login, say so.
- Ctrl-C: kill the local child or close the SSH session; the remote login dies
  with it. Closing a PTY-less exchange signals nothing remote (QA found
  a remote login still waiting on mini), so the login runs under
  `TIED_TO_STDIN`: it reads stdin through a FIFO and dies at its EOF, and lf
  keeps stdin open until the login exits.

## Constraints held

LOO-913 stands: lf chooses no login, switches nothing, polls no quota, refreshes
no provider token. Sign-in completes in the provider's flow with the unmodified
client. Nothing Loopflow operates sees a credential. For Codex lf moves no secret;
for Claude it forwards the person's own setup token by name, per launch,
from Doppler. Independent of LOO-926;
the two meet only at `forward::deliver`'s call sites. PR #1966 adds
`HarnessKind` and a per-harness trait: if it merges first, the sign-in command,
status probe and logout live on that trait instead of a `match` in
`machine/`. Slices 1–2 used the `match` (#1966 still open 2026-11-17); whichever
slice follows its merge moves them onto the trait. Still open after slice 3
(checked 2026-11-17): slice 4 stays on the `match` unless it merges first.

## Exclusions

- An agent-driven browser approver. The command seam is built; what sits
  behind it is not, and whether the providers' pages tolerate automation is
  untested.
- Checking that the approving account is an intended one. lf keeps no account.
- Pools of rental-only Codex logins restored and written back.
- API-key billing paths beyond today's OpenCode `--api-key`.

## What goes

- `machine/forward.rs`: `AGENT_LOGIN_ENV`, `AGENT_LOGIN_FILES`,
  `carries_agent_login`, `read_for_rental` and
  `a_rental_needs_a_forwarded_claude_or_codex_login`; its two callers in
  `rental.rs` become the prepare sign-in.
- `provider_auth/operations.rs`: `browser_profiles`, `remember_browser_profile`,
  `bootstrap_access_profile`, `verify_chrome_profile_login` and the
  interactive profile picker; `account_status.rs` folds into `machine logins`;
  `engine/account.rs`.
- `store/account_labels.rs`: `set_auth_browser_profiles`,
  `list_auth_browser_profiles`; `AuthBrowsers` and its validation in
  `config/settings.rs`; the `auth_browsers` catalog row.
- `lf account` family and its served routes, fixtures, Swift cases and tests
  (`auth_tests.rs`, `global_commands.rs`, `cli_discovery.rs`, `served_reads.rs`
  cases).
- `session_launch_contract.rs` "Rentals authenticate from names" test: becomes
  a per-run secret forward test without agent-login names.
- `credentials::connect`'s `email` parameter and its matching.
- `lf machine credentials inspect` and its handler; `lf machine connect` and
  its `MachineCommand::Connect` variant (renamed, not aliased).

Must survive: `forward.*` for per-run secrets; service transfers' identity
checks on both ends; LOO-913's fallback and wait rules.

## Slices (all built; history in git)

1. Remote sign-in over a PTY-less SSH exchange (`machine/login.rs`); taught
   that stdin pass-through works and error lines must come from stderr.
   Claude's paste-code relay built here was deleted by the rescope.
2. Codex device login, command approver, `lf machine logins [--json]`;
   `auth_browsers`, `AccessProfile` and the profile picker deleted.
3. `login::probe` and the launch check (`_run_signed_in`, 2 s, fail-open;
   measured `claude auth status --json` 0.34–0.39 s, `codex login status`
   0.02–0.05 s); `--logins` lives on the `ondemands.<name>` config template;
   `AGENT_LOGIN_ENV`/`read_for_rental` deleted.
4. `lf machine logout`, `--refused` (`SqliteStore::unanswered_refusal`),
   `lf account` and its routes, fixtures and Swift calls deleted.
5. Rescope: Claude by forwarded token (see top).

PR #1966 (`HarnessKind` trait) was still open at slice 4; the sign-in,
probe and logout stay on the `match` in `machine/login.rs` unless it merges
first.

## Built: one set of providers that reach other Machines (concept review)

`login::reaches_other_machines` (Codex, GitHub, Linear, OpenCode) is
the one set: `login::require_reach` refuses the others before any SSH (the CLI
calls it per named Machine before `resolve_target`, which already asks the
Machine's lf version over SSH; `login::login` calls it again), rental
`prepare`'s default goes through `rental::sign_in_order`, `ondemand add
--logins` rejects the rest, and `credentials::connect`'s catch-all is
`unreachable!`. Asana and Doppler sign in on each Machine itself; Claude
reaches others by forwarded token.
Tests: `machine_commands::services_that_sign_in_here_only_never_reach_another_machine`,
`rental::tests::a_rental_signs_in_once_to_each_provider_that_reaches_it_github_first`.

## Done: Claude's token fails like a login (concept review, 2026-11-23)

Built as below (`Forwarded::lacks_login` carries the left-out name to
`logins`). Checks run: `machine::forward::` unit, `session_launch_contract`
`a_signed_out_machine_refuses…`, `machine_commands::a_command_approver…`,
`documented_commands::` — all pass. Clippy left to gate.


QA (`scratch/qa.html`, "rough, worst first" 1): with `forward.env:
[CLAUDE_CODE_OAUTH_TOKEN]` set as the docs say, any launch from a shell
without the value (no `doppler run`) stops at `forward::read`'s "forward.env
names CLAUDE_CODE_OAUTH_TOKEN, which has no value on this Machine", Codex
launches included, and `lf machine logins mini` shows Claude `signed out`
with no reason. The token is Claude's login on other Machines; a missing login
means "Claude signed out there", which the launch check already handles per
provider. **Made for review:** a missing value for `login::CLAUDE_TOKEN_ENV`
is not an error.

- `machine/forward.rs` `read`: a `forward.env` name equal to
  `login::CLAUDE_TOKEN_ENV` with no value here (env or Doppler) is left out of
  `Forwarded.env`; every other name stays required (main's rule).
- `login::sign_in_advice(Claude)` on another Machine: "give this Machine
  CLAUDE_CODE_OAUTH_TOKEN (`doppler secrets set …`, or run under
  `doppler run`) and name it in `forward.env`" — the target cannot tell which
  half is missing.
- `login::logins` remote: when `forward.env` names the token but `read` left
  it out, keep the target's row and set `account` to
  `CLAUDE_CODE_OAUTH_TOKEN has no value here`. No new `LoginState`.
- Rental `place`: unchanged code; it now reaches its own "no agent login"
  stop instead of `read`'s error.
- Tests: `forward.rs` unit test (named Claude token, no value → `Ok`, not
  carried; another missing name still errors); extend
  `session_launch_contract::a_signed_out_machine_refuses_the_launch_with_the_command_that_signs_it_in`
  so a Codex launch with the token named and unset runs, and a Claude one is
  refused with the new advice; the `a_command_approver_…` fixture checks the
  `has no value here` row. Docs: `docs/subscriptions.md` "Per-run secrets"
  and the forwarded-row sentence say a missing token reads as Claude signed out.

Passed over: a new `forwarded`-without-value state; forwarding the token
without a `forward.env` entry (drops the opt-in for third-party rentals);
making every missing `forward.env` value optional.

## Remaining (gate)

Implementation is complete. Left for gate and after:

- Gate's headless acceptance fixture (below); its parts exist as
  `machine_commands::a_command_approver_signs_machines_in_to_codex_without_a_terminal`
  (with the `forwarded` row) and
  `session_launch_contract::a_signed_out_machine_refuses_the_launch_with_the_command_that_signs_it_in`
  (Codex and Claude cases).
- Not tested, note in the PR: rental `place`'s no-agent-login stop and
  prepare's sign-in loop (no rental fixture reaches rent); local
  `lf machine login claude` (hands the terminal to the real client).
- Swift `ServerProtocolTests` — CI.
- By a person: `lf machine login codex mini` for real, and a Claude launch on
  mini with the Doppler token forwarded.
- Open in `scratch/questions.md`: whether a usage-limit refusal should count
  for `--refused`.

## Done when

On a fresh owned Machine and on a fresh rental, an agent run on each provider
starts signed in after at most one approval in the provider's own flow; a
Machine with no login is refused before launch with the command that fixes it;
`rg 'lf account' docs rust swift` finds no caller.

## Acceptance check at gate

Headless, fixture SSH target with fake clients:
`lf machine login codex fixture` with `approvers.codex.command` set → exit 0,
`lf machine logins fixture --json` shows codex signed in and, with
`forward.env: [CLAUDE_CODE_OAUTH_TOKEN]`, claude `forwarded`; with the fake
clients signed out, `lf --machine fixture -a codex …` names `lf machine login
codex fixture` and `-a claude` names `forward CLAUDE_CODE_OAUTH_TOKEN`, both
before any agent starts; with the token forwarded, the Claude launch runs.
By a person at gate: `lf machine login codex mini` for real, and a Claude
launch on mini with the Doppler token forwarded.

## Checks

- `cargo test -p loopflow-execution --lib machine::` — implement, per slice.
- Fixture SSH sign-in test (slice 1) — implement.
- Real sign-in on mini, both providers — a person, at gate.
- Swift `ServerProtocolTests` after removing account cases — CI.

## Docs (written first, 2026-11-17)

Changed: `docs/subscriptions.md` ("Sign in on any Machine" replaces "Remote
Machines and rentals"; "Choose who approves", "Rentals", "Per-run secrets",
"Service authentication"), `docs/security.md`, `docs/dev/machines.md`,
`docs/ops.md`, `docs/config.md` ("Sign-in approvers"), `docs/work.md`,
`docs/dev/codebase.md`, `docs/dev/reference.md`, `README.md`, builtin
`init.md` and `repo_operate.md`. `server/README.md` and `security.md`'s service
transfer text, updated in slice 4.

Settled while writing: `logins` is `lf ondemand add … --logins
codex,github`, a field of the `ondemands.<name>` config template (slice 3
found templates in `~/.lf/config.yaml`, not the store). `LF_APPROVE_CODE` is set for any device-code page (Codex,
GitHub, OpenCode), not only Codex.

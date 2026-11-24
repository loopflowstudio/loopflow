# LOO-957: Sign Machines in the way each provider documents

## Rescope (2026-11-23) — supersedes the Claude design below

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
- `lf account` retirement, services, and Asana/Notion/Doppler staying here
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
`login::CLAUDE_TOKEN_ENV`. `login::sign_in_advice` replaces `sign_in_command`
and the agent's `_sign_in_advice` wrapper. Rental `place` computes the sign-in
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
Machine (no Machine named) and any other. For Claude and Codex it runs the provider's own login
**on the target Machine** and sends its approval page to this Machine's
browser. The credential is created where it is used, by the unmodified client,
and never crosses the network. `lf account` is gone; services (GitHub, Linear,
Asana, Notion, Doppler, OpenCode) sign in through the same command.

## Demo

```bash
lf set --global approvers.claude.chrome Work
lf machine login claude mini
#   Approve Claude for mini in Chrome (Work), then paste the code it shows:
#   > ****
#   mini: Claude signed in as jack@… (max)
lf machine login codex mini
#   Approve Codex for mini in Chrome (Work) with code ABCD-1234
#   mini: Codex signed in (ChatGPT)
lf machine logins mini
#   claude   signed in   jack@… (max)
#   codex    signed in   ChatGPT
#   github   signed in   jack-heart
#   linear   signed out
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

`machine::sign_in(target, provider, approver)` where `target` is this Machine
or an added one. For Claude and Codex:

1. Start the login on the target: locally as a child, remotely as one SSH
   exchange (`bounded_ssh_args`, no PTY) running `BROWSER=true claude auth login`
   or `codex login --device-auth`. `BROWSER=true` keeps an owned Mac mini from
   opening a window of its own.
2. Read its output for the approval URL (and Codex's code). Nothing else in
   that output is shown or logged.
3. Hand them to the approver (below). For Claude, the approver returns the code
   the page shows; lf writes it to the login's stdin.
4. Wait for the login to exit, then ask the target who it is
   (`claude auth status --json`, `codex login status`) and print it.

Same implementation for here and there; the only difference is whether the
child is local or behind SSH. Signing in here also pastes a code instead of
using Claude's localhost callback — **made for review**: one path over a
slightly smoother local flow; `claude auth login` by hand stays available.

What this answers from the brief's open decisions:
- *Where a Claude setup token sits / "collect, store"*: nowhere. Claude keeps
  its own login on that Machine and refreshes it. lf relays a one-time PKCE
  code on stdin and stores nothing.
- *Codex write-back*: nothing to write back. No `auth.json` is copied; each
  Machine has its own login and its own refresh grant, as the CI guide asks.
- *Several unattended rentals*: each runs the flow once at prepare, approved by
  the approver command when one is set.

Services keep their current transfer in `machine/credentials.rs` (GitHub
token, refreshed Linear access token, fresh OpenCode device login), now reached
through the same command. Their local form is today's `lf account connect`
code (`provider_auth/operations.rs::connect`, `configure` for `--api-key`).

### The approver: one personal setting per provider

```yaml
approvers:
  claude: {chrome: Work}
  codex:  {command: "subpersona approve"}
```

`approvers: BTreeMap<Provider, Approver>`, Personal, HostLocal, in
`config/catalog.rs` (category Accounts). `Approver` is `Chrome(String)` (a
profile name or directory, resolved by `profile::resolve_local_chrome_profile`)
or `Command(String)`.

| Approver | What lf does |
| --- | --- |
| unset | opens the default browser (`open`/`xdg-open`); a person approves |
| `chrome: <profile>` | `open_chrome_profile`; a person approves |
| `command: <cmd>` | runs `sh -c <cmd>` with `LF_APPROVE_PROVIDER`, `LF_APPROVE_URL`, `LF_APPROVE_CODE` (Codex), `LF_APPROVE_MACHINE`; exit 0 is approved; for Claude its first stdout line is the code |

The setting covers every provider whose sign-in opens a browser: Claude and
Codex, and the services' OAuth and device pages (GitHub, Linear, Asana, Notion,
OpenCode), which `auth_browsers` chose for today.

A person pasting Claude's code needs a foreground terminal
(`credentials::require_foreground`); a command approver doesn't. `--chrome-profile`
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
Linear/Asana/Notion/Doppler/OpenCode). Kept because retiring
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
- Client missing on the target: "Claude Code is not installed on mini; …".
- Approver command exits non-zero, or the login exits non-zero: the login's
  last error line (never its URL or code), nothing written.
- Codex device code disabled for the account: Codex's own error, plus "enable
  device code login in ChatGPT security settings".
- Approval not finished in 15 minutes (Codex's code lifetime; Claude uses the
  same deadline): kill the login, say so.
- Ctrl-C: kill the local child or close the SSH session; the remote login dies
  with it. Closing a PTY-less exchange signals nothing remote (QA found
  `claude auth login` still waiting on mini), so the login runs under
  `TIED_TO_STDIN`: it reads stdin through a FIFO and dies at its EOF, and lf
  keeps stdin open until the login exits.

## Constraints held

LOO-913 stands: lf chooses no login, switches nothing, polls no quota, refreshes
no provider token. Sign-in completes in the provider's flow with the unmodified
client. Nothing Loopflow operates sees a credential. The only secret lf moves
for Claude/Codex is the one-time code, on SSH stdin. Independent of LOO-926;
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

## Slices

1. **Claude on another Machine, end to end.** `lf machine login claude
   <machine>` with the approver unset or `chrome:` (the rename lands here, so
   no slice ships `connect` for Claude): SSH `claude auth login`, parse
   the URL, open it, read the pasted code, write it to stdin, print
   `claude auth status`. Test with a fake `claude` on a fixture SSH target that
   prints the probed output and records stdin; assert the code reaches the
   remote and never appears in lf's output or logs. This is the slice that
   could prove the plan wrong: no PTY, stdin pass-through over
   `bounded_ssh_args`, and a real sign-in against mini done by a person at gate.
   **Built (2026-11-17).** `machine/login.rs`: `login::login(machine, provider, …)`
   where a Machine with route `local` is this one, so here and there share
   `Login` (child or one `bounded_ssh_args` exchange, `BROWSER=true`).
   `lf machine connect` is gone; `lf machine login <provider> [<machine>…]`
   takes Claude here or remote, services remote through `credentials::connect`
   (its `email` parameter deleted) and here through `connect_target`, `--api-key`
   here only. `Approver` has `Browser` and `Chrome`. `claude_status` is the
   probe slices 2–3 reuse. Fixture: `machine_commands::claude_login_*` (the
   PTY helper now makes the terminal controlling and captures what it shows).
   Taught: stdin pass-through over a PTY-less SSH exchange works; the error
   line must come from stderr, since stdout's unterminated paste prompt flushes
   last. Codex still errors with "not built yet" until slice 2.
   Remaining for a person at gate: a real `lf machine login claude mini`.
2. **Codex, the command approver, this Machine as target.** **Built
   (2026-11-17)**, on the `match` in `machine/login.rs` (PR #1966 still open).
   `codex login --device-auth` on either target: lf reads the URL and the
   `ABCD-1234` line (probed against codex-cli 0.169.0), sends both to the
   approver and waits for Codex's own poll; failures add the ChatGPT
   device-code hint. `approvers.<provider>` is `config::placement::Approver`
   (`Chrome`/`Command`, one type; `--chrome-profile` becomes a one-run
   `Chrome`); `login::send` is the one dispatch, also used by service sign-in
   here and OpenCode's fresh device login. A command approver needs no
   terminal, for Claude too. `lf machine logins [<machine>] [--json]`: Claude
   and Codex from their clients, services from the store; another Machine
   answers with its own `lf machine logins --json` over `remote_command`.
   Deleted: `auth_browsers` (catalog row, validation, the old store export in
   `config/migrate.rs`), `AccessProfile`, `AuthBrowserBinding`, `ProfileId`,
   their store reads/writes and the interactive profile picker. Fixtures:
   `machine_commands::command_approvers_sign_machines_in_to_codex_and_claude_without_a_terminal`,
   `machine_commands::a_refusing_approver_leaves_the_machine_signed_out`.
   Not yet: the logins rows carry no last refusal (slice 4, with `--refused`);
   the probes have no timeout (slice 3 bounds them for the launch check);
   `lf machine credentials inspect` still answers service transfers (slice 4).
3. **Launch check and rentals.** **Built (2026-11-17)**, still on the `match`
   (#1966 open). `login::probe(machine, provider, deadline) -> LoginRow` is the
   one probe: `logins`, a finished sign-in and the launch check read it; the
   private `Status` and public `ClaudeStatus`/`claude_status` are gone. Every
   `run_agent` attempt goes through `_run_signed_in`: a definite signed-out
   (`login::signed_out_here`, the launch's env, 2 s deadline, fail-open) is an
   `AgentFailure::CredentialInvalidated` before the agent starts, so the order
   moves to the next provider or stops with `<Machine> has no Codex login; run
   `lf machine login codex <Machine>``. The launcher passes the name it gives
   the target in `LF_MACHINE_NAME` (`machine::task::forwarding_environment`);
   a mid-run revocation prints the same advice. Measured here: `claude auth
   status --json` 0.34–0.39 s, `codex login status` 0.02–0.05 s, inside the
   0.5 s p95 bound, so the check stays on every launch. `--logins` on
   `lf ondemand add` is a field of the `ondemands.<name>` config template, not
   a store column: templates live in `~/.lf/config.yaml` (the plan's "store"
   premise was wrong), so no migration. Rental prepare signs in to that list,
   else every provider `logins` shows signed in here, GitHub first for the
   clone; the planning-transport check for Linear is gone. Deleted:
   `AGENT_LOGIN_ENV`, `AGENT_LOGIN_FILES`, `carries_agent_login`,
   `read_for_rental` and its test; `lf ondemand rent` no longer refuses
   without a forwarded login. Fixture:
   `session_launch_contract::a_signed_out_machine_refuses_the_launch_with_the_command_that_signs_it_in`
   (remote and here), and the forward test now carries a generic per-run secret
   (`forwarded_secrets_reach_a_remote_launch_only_by_name`). Not tested: rental
   prepare's sign-in loop end to end (no rental fixture reaches prepare).
4. **Retire `lf account`.** **Built (2026-11-17)**, on the `match` (#1966
   open). `lf machine logout <provider> [<machine>…]`: Claude and Codex run
   their own logout here or over SSH and the probe must then read signed out;
   services here go through `operations::disconnect`, remotely through the
   target's own `lf machine logout`. `LoginRow.refused` is
   `SqliteStore::unanswered_refusal`: the provider's latest
   `agent_limit_recoveries` row with no `kind='agent'` process of it started
   since (range scan on `processes_recent`). `--refused` reads every added
   Machine's `logins --json` and signs in those reading signed out or refused;
   no sign-in time is recorded, so a refused Machine stays selected until a
   launch there. `logins` service rows read the client's own record first
   (gh's `hosts.yml` user, OpenCode's active export entry), else the store;
   service transfers' pre-check reads the target's `logins --json`, and
   `lf machine credentials inspect` is gone (the receiving end keeps its
   identity checks). Deleted: `Commands::Account`, `AccountCommand`,
   `parse_service_provider`, navigation's leading-value case,
   `commands/account.rs`, `engine/account.rs`, `account_status.rs`,
   `operations::served`/`narrate!`, `cached_status`, the `account.*` served
   routes and five DTO fixtures, Swift `accountList/Connect/Disconnect`.
   `operations::connect`/`disconnect` take a `Provider`. Fixtures:
   `machine_commands::refused_signs_in_again_only_the_machines_that_signed_out_or_refused_a_launch`,
   `auth_tests::service_auth_survives_subscription_removal` (now through
   `machine login --api-key`/`logout`/`logins`), storage
   `a_refusal_stands_until_its_provider_launches_again`.
   `rg 'lf account' docs rust swift` finds nothing. Not run here: Swift
   `ServerProtocolTests` (CI); a real `lf machine login claude mini` and
   `codex mini` (a person at gate).

## Built: one set of providers that reach other Machines (concept review)

`login::reaches_other_machines` (Claude, Codex, GitHub, Linear, OpenCode) is
the one set: `login::require_reach` refuses the others before any SSH (the CLI
calls it per named Machine before `resolve_target`, which already asks the
Machine's lf version over SSH; `login::login` calls it again), rental
`prepare`'s default goes through `rental::sign_in_order`, `ondemand add
--logins` rejects the rest, and `credentials::connect`'s catch-all is
`unreachable!`. Asana, Notion and Doppler sign in on each Machine itself.
Tests: `machine_commands::services_that_sign_in_here_only_never_reach_another_machine`,
`rental::tests::a_rental_signs_in_once_to_each_provider_that_reaches_it_github_first`.

## Remaining (gate)

All four slices are built on the `match` (#1966 still open). Left:

- Gate's headless acceptance fixture (below); its parts exist as
  `machine_commands::command_approvers_sign_machines_in_to_codex_and_claude_without_a_terminal`
  and `session_launch_contract::a_signed_out_machine_refuses_the_launch_with_the_command_that_signs_it_in`.
- Rental prepare's sign-in loop has no end-to-end test (no rental fixture
  reaches prepare); note it in the PR.
- Swift `ServerProtocolTests` — CI.
- A real `lf machine login claude mini` and `codex mini` — a person at gate.
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
claude,github`, a field of the `ondemands.<name>` config template (slice 3
found templates in `~/.lf/config.yaml`, not the store). `LF_APPROVE_CODE` is set for any device-code page (Codex,
GitHub, OpenCode), not only Codex.

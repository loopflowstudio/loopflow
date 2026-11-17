# LOO-957: Sign Machines in the way each provider documents

## Outcome

`lf machine connect [<machine>] <provider>` is the one sign-in command, for this
Machine and any other. For Claude and Codex it runs the provider's own login
**on the target Machine** and sends its approval page to this Machine's
browser. The credential is created where it is used, by the unmodified client,
and never crosses the network. `lf account` is gone; services (GitHub, Linear,
Asana, Notion, Doppler, OpenCode) sign in through the same command.

## Demo

```bash
lf set --global approvers.claude.chrome Work
lf machine connect mini claude
#   Approve Claude for mini in Chrome (Work), then paste the code it shows:
#   > ****
#   mini: Claude signed in as jack@… (max)
lf machine connect mini codex
#   Approve Codex for mini in Chrome (Work) with code ABCD-1234
#   mini: Codex signed in (ChatGPT)
lf machine logins mini
#   claude   signed in   jack@… (max)
#   codex    signed in   ChatGPT
#   github   signed in   jack-heart
#   linear   none
lf --machine mini -a codex implement      # on a mini with no Codex login:
#   mini has no Codex login; run `lf machine connect mini codex`
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

A person pasting Claude's code needs a foreground terminal
(`credentials::require_foreground`); a command approver doesn't. `--chrome-profile`
on `lf machine connect` overrides the setting for one run. A persona switcher
rewrites `approvers.*`; an agent-driven browser is a command. lf keeps no
account and no account-to-profile table.

This replaces `auth_browsers` (`default` list plus a per-account `accounts`
map) and the `access_profiles` store rows it pointed at. The branch stops
reading `auth_browsers`; the released lf keeps reading its own key, and
unknown keys only warn, so neither breaks the other. No fallback from old to
new: setting `approvers` once is the migration. The `access_profiles` table
stays (a drop is a staged contract); only its code goes.

### Which providers a Machine is signed in to

`logins: Vec<Provider>` on `machines.<name>`, `pools.<name>` and
`ondemands.<name>` declarations. A Machine's list is its own declaration, else
its pool's or on-demand's, else every provider signed in here. It does not
follow a request's agent selection. A rental takes it from its on-demand
template — **made for review**: of the two homes named ("in the launch
settnig and persisted for later or the pool itself"), the template needs no
new flag and no store column.

On the target, `own_logins: true | [providers]` (Personal, HostLocal) asserts
its own logins; `lf machine connect` and rental prepare skip those and say so.

Rental `prepare` (`lf/commands/rental.rs`) already signs in GitHub and Linear;
it signs in the whole list instead. That needs at most one approval per
provider, or none with command approvers.

### Every remote launch checks first

The harness gains a native-login probe (`claude auth status`,
`codex login status`) returning signed-in, signed-out or unknown. Before a
launch starts an agent of provider P, a definite signed-out is handled like a
revoked login (`agent/mod.rs`, `AgentFailure::CredentialInvalidated`): skip P's
models, continue on the next provider in the order, and with none left stop
with `_sign_in_advice`, which now names `lf machine connect <machine> <provider>`
(`lf machine connect <provider>` here). Unknown (probe missing, timed out,
unparseable) launches anyway: lf never refuses a working login on its own
misreading.

It runs on every launch, local too: one rule, no "am I remote" branch. Cost
bound: under 0.5 s added to a headless launch at p95, measured at implement;
past that it moves inside the SSH launch preamble only.

### Seeing and retrying

`lf machine logins [<machine>] [--json]` replaces `lf account [<service>]`:
per provider, `signed in | none | own`, with the account where the client
reports one (Claude email and plan; Codex none). Remote reads run
`lf machine logins --json` on the target over the existing
`credentials::remote_command`. It also reports each provider's last refusal on
that Machine (login missing or usage limit, from the recorded limit
recoveries) with its time.

`lf machine connect --refused [<provider>]` runs the sign-in on every added
Machine whose last launch of that provider was refused after its last sign-in.
The approving browser picks the account, so a retry needs no account switch to
be meaningful. This is the "resend token for all failed machines" command,
without a token to resend.

`lf machine disconnect [<machine>] <provider>` runs the provider's own logout
(`claude auth logout`, `codex logout`, `gh auth logout`, the stored token for
Linear/Asana/Notion/Doppler/OpenCode). Kept because retiring
`lf account disconnect` would otherwise drop it.

## Surfaces

| Surface | Change |
| --- | --- |
| `lf machine connect [<machine>] <provider>` | target optional; Claude, Codex and every service; `--chrome-profile`, `--api-key` (OpenCode), `--refused`; the `email` positional goes |
| `lf machine logins`, `lf machine disconnect` | new; replace `lf account`, `lf account disconnect` |
| `lf machine credentials inspect/receive` | stay as the remote end; `inspect` gains Claude/Codex via the probe |
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
  with it.

## Constraints held

LOO-913 stands: lf chooses no login, switches nothing, polls no quota, refreshes
no provider token. Sign-in completes in the provider's flow with the unmodified
client. Nothing Loopflow operates sees a credential. The only secret lf moves
for Claude/Codex is the one-time code, on SSH stdin. Independent of LOO-926;
the two meet only at `forward::deliver`'s call sites. PR #1966 adds
`HarnessKind` and a per-harness trait: if it merges first, the sign-in command,
status probe and logout live on that trait instead of a `match` in
`machine/`; rebase onto it before slice 2.

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

Must survive: `forward.*` for per-run secrets; service transfers' identity
checks on both ends; LOO-913's fallback and wait rules.

## Slices

1. **Claude on another Machine, end to end.** `lf machine connect <machine>
   claude` with the approver unset or `chrome:`: SSH `claude auth login`, parse
   the URL, open it, read the pasted code, write it to stdin, print
   `claude auth status`. Test with a fake `claude` on a fixture SSH target that
   prints the probed output and records stdin; assert the code reaches the
   remote and never appears in lf's output or logs. This is the slice that
   could prove the plan wrong: no PTY, stdin pass-through over
   `bounded_ssh_args`, and a real sign-in against mini done by a person at gate.
2. **Codex, the command approver, this Machine as target.** Device auth on
   either target; `approvers` setting and `LF_APPROVE_*` contract; `machine
   logins`. Remove `auth_browsers` and the access-profile code.
3. **Launch check and rentals.** The harness login probe feeding the revoked-
   login path with the new advice; `logins` on machines/pools/on-demands;
   rental prepare signs in the list; `own_logins`; delete the forward agent-
   login names and `read_for_rental`.
4. **Retire `lf account`.** Services through `lf machine connect`, `disconnect`,
   `--refused`; delete the command family, routes, fixtures, Swift cases;
   docs and builtin skills.

## Done when

On a fresh owned Machine and on a fresh rental, an agent run on each provider
starts signed in after at most one approval in the provider's own flow; a
Machine with no login is refused before launch with the command that fixes it;
`rg 'lf account' docs rust swift` finds no caller.

## Acceptance check at gate

Headless, fixture SSH target with fake clients:
`lf machine connect fixture claude` with `approvers.claude.command` set to a
script printing a code → exit 0, fake remote `claude` received the code on
stdin, `lf machine logins fixture --json` shows claude signed in; then with
the fake Codex signed out, `lf --machine fixture -a codex …` exits non-zero
before any agent starts, naming `lf machine connect fixture codex`.
By a person at gate: `lf machine connect mini claude` and `… codex` for real.

## Checks

- `cargo test -p loopflow-execution --lib machine::` — implement, per slice.
- Fixture SSH sign-in test (slice 1) — implement.
- Real sign-in on mini, both providers — a person, at gate.
- Swift `ServerProtocolTests` after removing account cases — CI.

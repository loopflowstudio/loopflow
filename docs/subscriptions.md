---
layout: default
title: Subscription Management
---

# Subscription Management

Connect Claude and Codex logins once, then route each repository through them:

```bash
lf account connect claude personal@example.com --chrome-profile personal@example.com
lf account connect codex work@example.com --chrome-profile work@example.com
lf account

lf account route set claude personal@
lf account route set codex work@ personal@
lf account route
```

Loopflow manages Claude and Codex subscription logins as separate identities.
OpenCode Zen, GitHub, and Linear each use one effective credential instead of a
routable subscription catalog.

`lf account` refreshes managed identity and capacity, then reports next actions.
Use `lf account --cached` for offline inspection with no provider
contact and no state writes. Unknown capacity stays unknown. JSON retains dated
observations and reset times; expired observations do not prove current capacity.
`lf account PROVIDER` limits the report to one provider.

## Connect an identity

`lf account connect` registers an existing provider login. It does not create a
Claude or Codex account.

```bash
lf account connect claude personal@example.com --chrome-profile personal@example.com
lf account connect codex work@example.com --chrome-profile work@example.com

lf account connect claude personal@example.com --import # adopt the ambient Claude login
lf disconnect claude personal@
```

Connect creates a **local managed identity**, not a new Claude or Codex account:

1. Loopflow derives a stable, path-safe account ID from the requested login.
   The ID is a storage key, not another username or credential.
2. The provider login runs in a private staging home. The chosen Chrome profile
   opens the provider's authorization page, but its browser profile is not
   copied.
3. Loopflow matches Codex's `account/read` email to the staged credential and
   reads its per-user subject from that credential. For Claude, it reads
   `/api/oauth/profile` account email and UUID. The observed email must match
   the requested email. A login already held by another managed account is
   refused, naming both accounts.
4. The verified provider credential moves into
   `~/.lf/accounts/<provider>/<account-id>/`.
5. Loopflow writes the observed email, user identity, plan and operating state to
   `~/.lf/loopflow.db`.

Codex requests a callback URL from `account/login/start` in its app-server,
which disables Codex's own browser opening. Loopflow opens that URL in the saved
Chrome profile and waits for the matching login completion before checking and
installing the staged identity. Cancellation or failed login keeps the existing
credential.

Claude opens its native browser callback route and completes after approval.
If the callback cannot reach this terminal, type `m` then Enter to open the
provider's manual route in the same profile; code input is hidden. Headless
connect never consumes stdin as an authorization code.

Select a Chrome profile once for each target:

```bash
lf account connect linear --chrome-profile Work
lf account connect linear              # reuse Work
lf account connect claude personal@    # reuse this account's profile
```

Saved names and ordered profile choices remain reusable. A first interactive
connection offers local Chrome profiles; headless connection requires
`--chrome-profile` when no choice is saved. A new choice need not have a Google
login. Existing expected-login constraints remain enforced. Browser selection is
remembered only after successful authorization, independently for each target.

If verification fails, the staging login is discarded and an existing identity
is left unchanged. `lf account connect claude <email> --import` is the explicit
exception: it copies
the ambient Claude login from `~/.claude` or the macOS Keychain into a new
isolated account home, then performs the same login verification.

The provider home holds the provider CLI's authentication and session state.
Loopflow may link shared settings, skills, and plugins from `~/.claude` or
`~/.codex`; it does not share credential or session files between managed
identities. The database stores health, routing state, usage observations,
access-profile bindings, and provider-session pins.

Commands identify an account by its full login email or an unambiguous email
prefix. The path-safe internal account ID is a storage key, not a user-facing
selector. An older account with no email can be selected by its ID solely to
set its expected email before reconnecting. `--login-email` refuses to relabel a
credential whose observed email disagrees.

An access profile records which Chrome profile can authenticate an identity:

```bash
lf account set claude personal@ --chrome-profile Personal --chrome-profile Work
lf account set linear --chrome-profile Work
lf account --details
```

The profile is an authentication venue, not the identity that spends provider
usage. It is never a run-time account selector.

Claude and Codex conversations run in the provider's ordinary home, signed in as
one stored account at a time, so plain `claude --resume` and `codex resume` find
them. See [Switch the shared account](#switch-the-shared-account). Launches
under `--isolate` run in the selected account's own home through
`CLAUDE_CONFIG_DIR` or `CODEX_HOME`; the provider CLI reads and refreshes its
credential there. Loopflow records health and session ownership against the
account ID, and reopens each conversation in the home it started in.

## Inspect account state

```bash
lf account                       # refresh managed identity and usage
lf account claude                # refresh one provider
lf account --cached              # offline managed and local evidence
lf account --details --json      # sources, saved browsers, full timestamps
```

Status lists stable account IDs beside full usable logins. Codex rows show the
credential's observed login, and report mismatches or shared
logins with a reconnect command even during cached inspection. Verification only
accepts usage after the server and native identity agree. Managed accounts and
local service credentials have separate sections: an expired local token says
nothing about a managed account. Missing local tokens leave ambient auth
uninspected. Cached inspection reads account state without starting a provider,
decrypting local tokens or creating an encryption key. The CLI records the
command's Process in its Machine; this can initialize an empty store, but creates no
account, route or conversation. Local token metadata is cached evidence, not server acceptance;
live status reports local server verification unavailable.

`auth status` persists recognized managed subscription windows before
printing percentages, reset times and plan. Each window keeps its own observation
age and source; omitted or unavailable windows retain older evidence. A passed
reset displays usage as unknown until refreshed. Current windows say both
`N% used` and `M% left`. JSON retains dated window observations; consumers must
check `resets_at` before treating a recorded percentage as current. `lf usage`
reports recorded provider token/cost usage separately.

JSON contains `accounts` and optional `browser` details. Each row has `scope`
(`managed` or `local`), provider, account ID, login, cached
credential state, verification, windows, diagnostic and recovery. Verification
is `accepted`, `rejected`, `unavailable`, or `not_checked`. Window rows retain
`observed_at`, `resets_at`, source and plan; `verified_windows` identifies only
this invocation's new observations. Null means unknown. A successful report
does not imply successful authentication. Cron-host requires accepted managed
evidence:

```bash
lf account --json |
  jq -e 'any(.accounts[]; .scope == "managed" and .verification == "accepted")'
```

Claude's cached login metadata does not establish a freshly observed plan.
Until the usage response supplies confirmed plan evidence, its new observations
leave plan unknown. Codex retains the plan from `account/read`, including when
no usage windows are returned. Healthy automatic Codex candidates prefer an
observed Pro plan, then Plus; explicit preferences and Session pins take priority.
A plan does not establish remaining capacity.

Cached account inspection starts no provider and changes no stored state. Missing
credentials remain visible with older usage and the `lf account connect` recovery
command. Unreadable credentials are distinguished from missing files. Verification
records provider rejection as missing; locally detected identity mismatches are
reported without changing cached state. Unavailable usage leaves credential state
unchanged. Neither success nor rejection clears a routing cooldown or changes
account configuration. Provider routes skip missing, disabled, cooling and
limited accounts and continue to the next candidate.

A credential replacement detected during verification makes that result
unavailable and retains prior account evidence. Claude refresh coordination with
native sessions is still under development; these comparisons do not serialize
native refresh or credential writes.

An active usage window at 95% or above demotes that account behind candidates
below the threshold. Codex plan preference then applies; declared route order
decides remaining ties. A provider session stays
pinned to the account that created it so a resume does not silently switch
identities.

Claude's cached identity is bound to the credential bytes checked by the profile
endpoint. Cached status reports changed or previously unverified credentials as
unknown; stale `.claude.json` metadata never establishes identity. Live status
and connect save that binding. Routing checks expected email, user UUID and
duplicate logins, requesting a profile observation when the credential changed.
Recorded Claude and Codex launches both require their owning account catalog.

Account state remains in the Machine database.

Control automatic routing per account:

```bash
lf account set claude personal@ --paid-through 2026-08-14
lf account set claude personal@ --routing explicit-only
lf account set claude personal@ --clear-cooldown
```

Clearing cooldown retains credential state, aggregate utilization and every
observed window. It does not reset provider quota.

An `explicit-only` identity runs only when a route or command selects it.
`disabled` identities never run. Once `paid-through` passes, an otherwise
automatic account behaves as `explicit-only` until the date is cleared.

## Route a repository

A repository account route is an ordered list of subscription logins to try for
one provider in one repository:

```bash
lf account route set claude personal@ work@
lf account route set codex work@ personal@
lf account route set codex work@ --default
lf account route --json
```

Show lists eligible launch candidates in order without selecting one. Explicit
`--repo` and `--default` work outside Git; a write without either requires a
repository origin.

It is account-selection metadata, not an SSH or network route, and it contains
no credential. Repository routes live in the local Loopflow database.

A managed launch tries the repository route, then the default provider route.
Without either route, every automatically eligible managed identity is a
candidate. With no managed candidate, the provider CLI can use its ambient
default login.

## Select accounts for one launch

Inspect a running worker's selected account:

```bash
lf mon list <run-id> --events
```

`provider_account_selected` records the actual account and attempt, including
headless Task steps before a provider session ID is available. Later attempts
retain earlier account evidence; a null account means ambient execution. Once
the provider reports its native identity, continuation must retain that identity
and selected account/native Machine. Requested account and actual selection are
different evidence. The event command above remains a transitional launch
interface; [cutover status](architecture-reference.md#cutover-status) records the
remaining history/account conversion.

Prefer an account while keeping the normal route as fallback:

```bash
lf --account codex=work@ implement
lf --account claude=personal@ --account codex=work@ review
```

Restrict the process tree to exact accounts:

```bash
lf --only-account claude=personal@ review
lf --only-account claude=personal@ --only-account codex=work@ implement
```

`--account` and `--only-account` are repeatable. An unqualified selector is
resolved independently for Claude and Codex. The two flags cannot be combined.
A provider omitted from `--only-account` is unavailable to that process tree.

Choose both providers once for a Flow or Task:

```bash
lf --only-account claude=personal@ --only-account codex=work@ flow code
lf --account claude=personal@ --account codex=work@ task run LOO-123
```

A Flow carries these choices to its steps. `--account` keeps fallback routing;
`--only-account` restricts the entire Flow. Run a fresh Flow to choose again.
Steps have no account overrides. Native Session affinity remains authoritative within that selection.
Local choices use the Machine's catalog directly and survive the initiating CLI's
exit, including logins installed on a remote machine.

## Switch the shared account

```bash
lf -a codex : "say hi"                    # runs in ~/.codex as the active account
codex resume                              # plain Codex sees that conversation
lf account codex use work@                # ~/.codex is now work@, for lf and codex
lf account route                          # shows the mode and the active account
lf --isolate --account codex=work@ -a codex : "say hi"   # stays in work@'s own home
lf account claude use work@               # the same for ~/.claude and plain claude
```

`lf account <provider> use` signs the provider's home in as a stored account.
It first saves the current login back to its
stored account, so a token the provider refreshed while active is kept; a login
Loopflow has never seen is kept as a new explicit-only account.

For Codex, `use` also restarts its shared background app-server if it still holds
another login, even when the file was already correct. Running turns get up to
five minutes to finish; a turn still running at restart is interrupted. The login
is installed before that wait, so new lf launches need not wait. lf's Codex
terminals use `--no-daemon` and keep their own login until restarted.
An unreadable daemon login returns an error without restarting it; the selected
login remains installed. Failed activity checks keep the grace period rather than
ending it early. Expiry warns that restarting may interrupt remaining turns.
Cancelling during the wait leaves the installed login in place; repeat `use` to
reconcile the daemon later.
This flag was verified with codex-cli 0.161.0; older versions without it reject
lf's terminal launches.

| | Codex | Claude |
|---|---|---|
| Where the home's login lives | `auth.json` | macOS Keychain item; `.credentials.json` elsewhere |
| An lf process already running | keeps the login it started with until it restarts | follows the switch |
| A turn in flight during a switch | may fail once; a headless run resumes it under the new account | continues |
| Whose login the home holds | read from the login | asked of Claude once after Claude refreshes it |

A Claude login names no person. After Claude refreshes the home's login,
Loopflow asks Claude whose it is before the next launch or switch and brings
that account's stored copy up to date. If it cannot ask, a switch keeps the
unrecognized login beside the stored accounts rather than discarding it.

A shared launch follows the active account while it is eligible and below the
95% threshold, and otherwise moves the home to the next account in the route.
`--account codex=work@` makes that account active before launching.
These launch-time switches do not restart Codex's shared daemon. If plain
`codex` keeps the old login, run `lf account codex use` with the selected account.

`--isolate` runs the invocation and its children in the selected account's own
home, unmoved by any switch. `isolate: true` in `.lf/config.yaml` makes that the
default, and `--shared` overrides it for one invocation. A Flow's steps keep
the mode it was launched with.

Codex's own conversation ID works wherever a Session ID does:

```bash
lf session connect 0199a213-81c0-7800-8aa1-bbab2a035a53   # the ID `codex resume` takes
lf session history 0199a213-81c0-7800-8aa1-bbab2a035a53
```

Connecting to a conversation plain Codex started brings it in as a Session with
no Task. Normal batch completion and closing the current connected interface
close its Codex process, leaving saved history available to plain `codex resume`.
`lf session connect` takes over a running conversation without restarting it;
the old attachment's exit cannot close the new owner's Codex process. Codex allows one
writer per conversation, so use `lf session connect` while it is still running.

`lf account route --json` reports `isolated` and `active_account` per provider.

A Codex home whose `config.toml` sets `cli_auth_credentials_store` to anything
but `file` cannot be switched; set it to `file` or launch with `--isolate`.

## Banked Codex resets

```bash
lf account codex
lf account redeem-reset codex work@example.com
# Retry an interrupted attempt with the key printed before its request:
lf account redeem-reset codex work@example.com --idempotency-key <same-key>
```

Live status shows the provider's available reset count and any returned credit
statuses and expiry times. `--cached` reports resets as unknown. A missing detail
list does not mean zero credits; the provider's count is authoritative.

Only `redeem-reset` spends a credit. It requires a named managed Codex login,
validates identity, reports usage before and after, and supports `--json`.
`--credit-id <id>` selects a particular returned credit; otherwise Codex chooses.
Status, routing and Flow launches never redeem resets.

Each attempt prints an idempotency key before sending the request. Reuse it
if the reply is lost. `reset` confirms a spend; `alreadyRedeemed` confirms that
the same attempt completed earlier. `nothingToReset` and `noCredit` report no
spend. If the following usage refresh fails, the command retains the confirmed
outcome and reports the after-state as unknown. It never guesses new capacity.
The [Codex app-server protocol](https://learn.chatgpt.com/docs/app-server#8-earned-rate-limit-resets-chatgpt)
owns these outcomes and retry semantics.

## Use subscriptions over SSH

```bash
lf machine connect mini github
lf machine connect mini codex work@example.com
lf machine connect mini claude personal@example.com --chrome-profile Personal
lf machine connect mini linear
lf --machine mini --account codex=work@ implement
```

Add the machine first with `lf machine add`. GitHub installs the laptop's selected
token in the target's gh configuration. Claude, Codex and Linear mint a fresh login
on the laptop; approve it in the laptop's browser. Claude and Codex stage the login
in a private temporary directory. The laptop's existing login is not replaced.
The target retains its own refresh credential and existing accounts.

An account-selected launch connects the requested account if missing. Invocation selectors resolve
against the laptop's managed logins;
full email identifies that login on the target. The remote invocation is restricted
to the requested accounts, so it cannot fall back to a different login. A simple
launch without selectors uses the laptop's configured provider route when available.
Read commands without account selectors do not connect accounts.

Transfers run in the foreground. `--batch` never starts a missing login, even
with a terminal attached. A headless launch needing a missing login reports
the connect command; it does not start browser authorization. SSH launches use
isolated, file-backed account homes, including on macOS. The target needs its own
provider CLI. Credential bytes travel on SSH stdin, never in argv or an exported
variable. `machine credentials inspect/receive` supplies the public receiving
commands. No broker, forwarded socket or laptop process owns the installed login.

`lf --machine` runs the target's `lf` in the saved repository. The target parses
the command; there is no extra `-- lf`. Remote process survival and reattachment belong to the separate detached-launch and relay work.

## OpenCode Zen

OpenCode Zen uses one credential rather than the managed subscription route:

```bash
lf account connect opencode
lf account connect opencode --api-key
```

Its stored credential applies to local OpenCode launches.
Subscription polling, repository account routes, and the 95% demotion threshold
do not apply.

## Rust API migration

This auth consolidation changes the published Rust library's source API. Crate
consumers must migrate; the removed interfaces have no compatibility aliases.

- Call `ProviderAuthService::start_auth(provider)` and `disconnect(provider)`
  without an event sink. `AuthEvent`, `AuthEventSink` and `no_event_sink` are
  removed. After opening the returned browser URL, await
  `wait_for_auth(provider)` once to receive that attempt's completion, including
  persistence errors. Do not use cached `status()` as completion evidence.
- Construct `AuthFlowResponse` with `completion: AuthCompletion`. Use
  `supports_authorization_code()` and `pending_supports_authorization_code()`
  in place of the former `requires` methods. Manual input support does not mean
  authorization requires a code. Chrome authorization-code scraping is removed;
  use provider completion or explicit manual submission.
- Replace `AccountAccessProfile` with `AuthBrowserBinding`. Its `account_id`
  is optional: `Some(id)` selects a managed account; `None` names local service
  scope. `AccessProfile::expected_login` is also optional. Store and SqliteStore
  expose `set_auth_browser_profiles` / `list_auth_browser_profiles` in place of
  the account-access methods. In the list method, a missing account filter lists
  both scopes; filter binding rows when selecting a local service.
- Include `plan: Option<String>` and `reset_credits: Option<RateLimitResetCredits>`
  when constructing `SubscriptionUsage`. Read
  the account's stored `home` instead of the removed `subscription::account_home`
  path constructor.
- Include `observed_credential_digest: Option<String>` in `ProviderAccount`.
  Pass the optional credential digest to `record_provider_account_identity`;
  an absent digest retains historical identity without asserting a Claude
  credential binding.
- Account selection lives in `provider_account::selection`; the SSH credential
  broker and lent-account API are removed. Launch local commands with `--account`
  or `--only-account`; a Flow carries those choices to its steps.
- Route candidates belong to the executing machine. Their JSON and text omit
  the former local/forwarded `source` field.

The forward database migration preserves existing account/profile identities,
ordered bindings and configured expected logins. Source API compatibility and
database preservation are separate contracts; local tests do not establish that
external crate consumers have migrated.

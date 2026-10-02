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
Use `lf account --cached` for offline inspection with no provider or broker
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

Codex conversations run in Codex's ordinary home, signed in as one stored
account at a time, so plain `codex resume` finds them. See
[Switch the shared account](#switch-the-shared-account). Claude launches, and
Codex launches under `--isolate`, run in the selected account's own home through
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
command's Exec in its Home; this can initialize an empty store, but creates no
account, route or conversation. An inherited account lease carries no cached identity catalog:
`--cached` reports forwarded identities as uninspected without contacting the
origin broker. Local token metadata is cached evidence, not server acceptance;
live status reports local server verification unavailable.

`auth status` persists recognized managed subscription windows before
printing percentages, reset times and plan. Each window keeps its own observation
age and source; omitted or unavailable windows retain older evidence. A passed
reset displays usage as unknown until refreshed. Current windows say both
`N% used` and `M% left`. JSON retains dated window observations; consumers must
check `resets_at` before treating a recorded percentage as current. `lf usage`
reports recorded provider token/cost usage separately. By default, status
reads forwarded identity metadata from the origin broker, without acquiring a remote credential or
verifying remote accounts. An unavailable broker leaves local evidence visible.

JSON contains `accounts`, optional `forwarded_accounts_diagnostic`, and optional
`browser` details. Unknown forwarded identities have no invented account rows.
Each row has `scope`
(`managed`, `local`, or `forwarded`), provider, account ID, login, cached
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

Account state remains in the Home database.

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
lf monitor list <run-id> --events
```

`provider_account_selected` records the actual account and attempt, including
headless Task steps before a provider session ID is available. Later attempts
retain earlier account evidence; a null account means ambient execution. Once
the provider reports its native identity, continuation must retain that identity
and selected account/native Home. Requested account and actual selection are
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
lf --account claude=personal@ --account codex=work@ --task LOO-123 flow start
```

Flow invocations save these choices and restore them for autonomous steps,
review children and later resumes. `--account` keeps fallback routing;
`--only-account` restricts the entire invocation. Resuming a saved invocation
keeps its captured choices; restart it to choose again. Steps have no account
overrides. Native Session affinity remains authoritative within that selection.
Local choices use the Home's catalog directly and survive the initiating CLI's
exit; a forwarded SSH credential still needs its origin broker.

## Switch the shared account

```bash
lf -m codex : "say hi"                    # runs in ~/.codex as the active account
codex resume                              # plain Codex sees that conversation
lf account use codex work@                # ~/.codex is now work@, for lf and codex
lf account route                          # shows the mode and the active account
lf --isolate --account codex=work@ -m codex : "say hi"   # stays in work@'s own home
```

`lf account use` is the only command that changes which stored account Codex's
home is signed in as. It first saves the current login back to its stored
account, so a token Codex refreshed while active is kept; a login Loopflow has
never seen is kept as a new explicit-only account. Running Codex processes keep
the login they started with until they restart.

A shared launch follows the active account while it is eligible and below the
95% threshold, and otherwise moves the home to the next account in the route.
`--account codex=work@` makes that account active before launching.

`--isolate` runs the invocation and its children in the selected account's own
home, unmoved by any switch. `isolate: true` in `.lf/config.yaml` makes that the
default, and `--shared` overrides it for one invocation. Flow invocations keep
the mode they were launched with.

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

`lf ssh` runs the target machine's `lf`. The target name is the argument
boundary: selectors before it are resolved on the origin; everything after it
is ordinary syntax for the target `lf`.

```bash
# Offer all origin accounts and let the target lf choose.
lf ssh my-company implement

# Prefer this exact identity from the origin.
lf ssh --account personal@ my-company implement

# Resolve this preference from the target's combined catalog.
lf ssh my-company --account work@ implement
```

There is no explicit `-- lf`. `lf ssh` does not run arbitrary remote programs;
use ordinary `ssh` for those. The target can be an SSH hostname or a Loopflow
Home ID. A Home ID resolves its current SSH address and makes the reached
machine prove its identity.

Without an outer selector, the origin offers every connected managed identity,
the facts governing its eligibility, and the current repository's account
routes. The target merges those with its own identities and route.

The target chooses in this order:

1. target-side `--account` preferences;
2. origin-side `--account` preferences;
3. the target repository route;
4. the forwarded origin repository route; and
5. the remaining eligible target and forwarded accounts.

Target-local accounts precede equivalent forwarded accounts when no explicit
preference distinguishes them. Health and usage rules apply across the merged
catalog.

An origin-side `--only-account` is resolved against origin identities before
SSH connects. The target can narrow that grant but cannot widen it. Account
inspection shows `local` or `forwarded` provenance. Forwarded identities are
read-only: connect, disconnect, and edit their routes on the machine that owns
them.

Launch selectors govern foreground work. A detached worker sheds forwarded state before detaching and selects from routes stored
on its own machine. Configure the target repository route for durable account
choice:

```bash
lf ssh my-company route set codex work@
lf ssh my-company --wave shipper wave/operate
```

The origin does not copy account homes or refresh credentials. It advertises
the catalog first and serves one access token only after the target selects a
forwarded identity. See [Security](/docs/security#what-crosses-ssh-for-a-subscription-account)
for the broker, process-lifetime, and remote trust boundary.

## OpenCode Zen

OpenCode Zen uses one credential rather than the managed subscription route:

```bash
lf account connect opencode
lf account connect opencode --api-key
```

Its stored credential applies to local OpenCode launches and foreground SSH.
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
- `AccountLeaseBroker` is internal to SSH forwarding; its `start_root` and
  `local_env_value` entry points are removed. Launch local commands with
  `--account` or `--only-account`; saved Flow invocations own those choices.

The forward database migration preserves existing account/profile identities,
ordered bindings and configured expected logins. Source API compatibility and
database preservation are separate contracts; local tests do not establish that
external crate consumers have migrated.

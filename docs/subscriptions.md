---
layout: default
title: Subscription Management
---

# Subscription Management

Connect Claude and Codex logins once, then route each repository through them:

```bash
lf auth connect claude personal@example.com --chrome-profile personal@example.com
lf auth connect codex work@example.com --chrome-profile work@example.com
lf auth status

lf auth route set claude personal@
lf auth route set codex work@ personal@
lf auth route show
```

Loopflow manages Claude and Codex subscription logins as separate identities.
OpenCode Zen, GitHub, and Linear each use one effective credential instead of a
routable subscription catalog.

## Connect an identity

`lf auth connect` registers an existing provider login. It does not create a
Claude or Codex account.

```bash
lf auth connect claude personal@example.com --chrome-profile personal@example.com
lf auth connect codex work@example.com --chrome-profile work@example.com

lf auth connect claude personal@example.com --import  # adopt the ambient Claude login
lf auth disconnect claude personal@
```

Connect creates a **local managed identity**, not a new Claude or Codex account:

1. Loopflow derives a stable, path-safe account ID from the requested login.
   The ID is a storage key, not another username or credential.
2. The provider login runs in a private staging home. The chosen Chrome profile
   opens the provider's authorization page, but its browser profile is not
   copied.
3. Loopflow asks the provider CLI which login completed authorization and
   requires it to match the requested email.
4. The verified provider credential moves into
   `~/.lf/accounts/<provider>/<account-id>/`.
5. Loopflow writes the verified login and non-secret operating state to
   `~/.lf/loopflow.db`.

Claude opens its native browser callback route and completes after approval.
If the callback cannot reach this terminal, type `m` then Enter to open the
provider's manual route in the same profile; code input is hidden. Headless
connect never consumes stdin as an authorization code.

Select a Chrome profile once for each target:

```bash
lf auth connect linear --chrome-profile Work
lf auth connect linear                          # reuse Work
lf auth connect claude personal@                # reuse this account's profile
```

Saved names and ordered profile choices remain reusable. A first interactive
connection offers local Chrome profiles; headless connection requires
`--chrome-profile` when no choice is saved. A new choice need not have a Google
login. Existing expected-login constraints remain enforced. Browser selection is
remembered only after successful authorization, independently for each target.

If verification fails, the staging login is discarded and an existing identity
is left unchanged. `lf auth connect claude <email> --import` is the explicit
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
selector.

An access profile records which Chrome profile can authenticate an identity:

```bash
lf auth set claude personal@ --chrome-profile Personal --chrome-profile Work
lf auth set linear --chrome-profile Work
lf auth status --details
```

The profile is an authentication venue, not the identity that spends provider
usage. It is never a run-time account selector.

At launch, Loopflow points the provider child at the selected account home with
`CLAUDE_CONFIG_DIR` or `CODEX_HOME`. The provider CLI reads and refreshes its own
credential there. Loopflow records health and session ownership against the
same account ID so fallback and resume do not silently change identities.

## Inspect account state

```bash
lf auth status                       # offline managed and local evidence
lf auth status claude --verify       # request managed Claude observations
lf auth status --details --json      # sources, saved browsers, full timestamps
```

Status lists stable account IDs beside full usable logins. Managed accounts and
local service credentials have separate sections: an expired local token says
nothing about a managed account. Missing local tokens leave ambient auth
uninspected. Cached inspection opens the database read-only, starts no provider,
does not decrypt local tokens or create an encryption key, and leaves an absent
store absent. An inherited account lease carries no cached identity catalog:
plain status reports forwarded identities as uninspected without contacting the
origin broker. Local token metadata is cached evidence, not server acceptance;
`--verify` reports local server verification unavailable.

`auth status --verify` persists recognized managed subscription windows before
printing percentages, reset times and plan. Each window keeps its own observation
age and source; omitted or unavailable windows retain older evidence. A passed
reset asks for refresh instead of implying zero usage. `lf usage` reports recorded provider
token/cost usage separately. With `--verify`, status reads forwarded identity
metadata from the origin broker, without acquiring a remote credential or
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
lf auth status --verify --json |
  jq -e 'any(.accounts[]; .scope == "managed" and .verification == "accepted")'
```

Claude's cached login metadata does not establish a freshly observed plan.
Until the usage response supplies confirmed plan evidence, its new observations
leave plan unknown. Codex retains the plan returned by its rate-limit response.

Cached account inspection starts no provider and changes no stored state. Missing
credentials remain visible with older usage and the `lf auth connect` recovery
command. Unreadable credentials are distinguished from missing files. Verification
records decisive rejection as missing; unavailable usage leaves credential state
unchanged. Neither success nor rejection clears a routing cooldown or changes
account configuration. Provider routes skip missing, disabled, cooling and
limited accounts and continue to the next candidate.

A credential replacement detected during verification makes that result
unavailable and retains prior account evidence. Claude refresh coordination with
native sessions is still under development; these comparisons do not serialize
native refresh or credential writes.

An active usage window at 95% or above demotes that account behind candidates
below the threshold. Declared route order decides ties. A provider session stays
pinned to the account that created it so a resume does not silently switch
identities.

Control automatic routing per account:

```bash
lf auth set claude personal@ --paid-through 2026-08-14
lf auth set claude personal@ --routing explicit-only
lf auth set claude personal@ --clear-cooldown
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
lf auth route set claude personal@ work@
lf auth route set codex work@ personal@
lf auth route set codex work@ --default
lf auth route show --json
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
lf runs <run-id> --events
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
lf auth connect opencode
lf auth connect opencode --api-key
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
- Include `plan: Option<String>` when constructing `SubscriptionUsage`. Read
  the account's stored `home` instead of the removed `subscription::account_home`
  path constructor.

The forward database migration preserves existing account/profile identities,
ordered bindings and configured expected logins. Source API compatibility and
database preservation are separate contracts; local tests do not establish that
external crate consumers have migrated.

---
layout: default
title: Subscription Management
---

# Subscription Management

Connect Claude and Codex logins once, then route each repository through them:

```bash
lf auth connect claude personal@example.com --chrome-profile personal@example.com
lf auth connect codex work@example.com --chrome-profile work@example.com
lf auth accounts

lf route set claude personal@
lf route set codex work@ personal@
lf route show
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

lf auth import claude --email personal@example.com  # adopt the ambient Claude login
lf auth disconnect claude --email personal@
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

If verification fails, the staging login is discarded and an existing identity
is left unchanged. `lf auth import claude --email <email>` is the explicit
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
lf profile create --chrome-profile personal@example.com --as personal
lf auth access set claude personal@ --profile personal
```

The profile is an authentication venue, not the identity that spends provider
usage. It is never a run-time account selector.

At launch, Loopflow points the provider child at the selected account home with
`CLAUDE_CONFIG_DIR` or `CODEX_HOME`. The provider CLI reads and refreshes its own
credential there. Loopflow records health and session ownership against the
same account ID so fallback and resume do not silently change identities.

## Inspect account state

```bash
lf auth status                   # every connected service
lf auth accounts claude          # cached subscription state
lf auth accounts --verify        # compare every credential with its provider
```

`auth accounts --verify` records a revoked credential as missing and prints
the exact `lf auth connect` recovery command. Provider routes skip missing,
disabled, cooling, and limited accounts and continue to the next candidate.

An active usage window at 95% or above demotes that account behind candidates
below the threshold. Declared route order decides ties. A provider session stays
pinned to the account that created it so a resume does not silently switch
identities.

Control automatic routing per account:

```bash
lf auth set claude personal@ --paid-through 2026-08-14
lf auth set claude personal@ --routing explicit-only
lf auth reset claude personal@
```

An `explicit-only` identity runs only when a route or command selects it.
`disabled` identities never run. Once `paid-through` passes, an otherwise
automatic account behaves as `explicit-only` until the date is cleared.

## Route a repository

A repository account route is an ordered list of subscription logins to try for
one provider in one repository:

```bash
lf route set claude personal@ work@
lf route set codex work@ personal@
lf route show
```

It is account-selection metadata, not an SSH or network route, and it contains
no credential. Repository routes live in the local Loopflow database.

A managed launch tries the repository route, then the default provider route.
Without either route, every automatically eligible managed identity is a
candidate. With no managed candidate, the provider CLI can use its ambient
default login.

## Select accounts for one launch

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

Launch selectors govern foreground work. A Wave resident that survives the SSH
command sheds forwarded state before detaching and selects from routes stored
on its own machine. Configure the target repository route for durable account
choice:

```bash
lf ssh my-company route set codex work@
lf ssh my-company start shipper
```

The origin does not copy account homes or refresh credentials. It advertises
the catalog first and serves one access token only after the target selects a
forwarded identity. See [Security](/docs/security#what-crosses-ssh-for-a-subscription-account)
for the broker, process-lifetime, and remote trust boundary.

## OpenCode Zen

OpenCode Zen uses one credential rather than the managed subscription route:

```bash
lf auth opencode
lf auth configure opencode
```

Its stored credential applies to local OpenCode launches and foreground SSH.
Subscription polling, repository account routes, and the 95% demotion threshold
do not apply.

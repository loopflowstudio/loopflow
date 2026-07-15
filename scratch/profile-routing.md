# Profile routing and account lifecycle

## End state

A Loopflow profile is the unit a repository routes through. Each profile binds
one host-local Chrome profile and one account per provider. Provider accounts
are reusable: multiple Loopflow profiles can deliberately point at the same
Claude or Codex account.

```text
repository
  default_profile: jackstah
  backup_profiles: [loopflow-eng, cadenza-eng]
                         |
                         v
profile -------------------------------+
  chrome profile                        |
  claude account -> provider account <--+-- shared health and billing state
  codex account  -> provider account <--+
```

The intended mappings are:

| Loopflow profile | Claude account | Codex account |
|---|---|---|
| `jackstah` | `jack@loopflow.studio` | `jack@loopflow.studio` |
| `jack` | `jack@loopflow.studio` | `jack@loopflow.studio` |
| `loopflow-eng` | `loopflow-eng@loopflow.studio` | `loopflow-eng@loopflow.studio` |
| `cadenza-eng` | `loopflow-eng@loopflow.studio` | `cadenza-eng@loopflow.studio` |

The existing `jackstah@gmail.com` Claude and Codex accounts remain registered
after they downgrade, but no automatic route uses them unless a profile maps to
them explicitly.

## Ownership

```rust
struct Profile {
    id: ProfileId,
}

struct ChromeProfileBinding {
    profile_id: ProfileId,
    host_id: HostId,
    chrome_directory: String,
    google_email: EmailAddress,
}

struct ProfileProviderAccount {
    profile_id: ProfileId,
    provider: Provider,
    account_id: ProviderAccountId,
}

struct RepoProfileRoute {
    repo_id: RepoId,
    default_profile: ProfileId,
    backup_profiles: Vec<ProfileId>,
}
```

Profile names are user-chosen routing identities. A Chrome binding is
host-local because `Profile 7` on one Mac means nothing on another host.
Provider accounts are keyed by provider plus normalized login email in the
user-facing model; credential-home paths use an opaque stable id rather than
putting email addresses in paths.

Repository routes are personal local state keyed by `RepoId`, not committed
`.lf/config.yaml`. Checked-in config is a team convention, while OAuth profile
names and backup order belong to the operator. `lf ssh` carries the resolved
route instead of requiring the remote host to have matching local state.

## Routing

For a provider launch, build candidates in this order:

```text
[default_profile] + backup_profiles
```

Resolve each profile to its account for the requested provider, discard
disabled or cooling-down accounts, deduplicate shared account ids, and select
the first remaining candidate. Pin both the profile and resolved provider
account to the provider child for its lifetime.

A rate-limit response updates the provider account's global health, not the
profile. If `jackstah` and `jack` both resolve Claude to
`jack@loopflow.studio`, exhausting it removes both candidates for Claude. The
router cannot mistake two names for two pools.

This also means `jack` is not useful as a backup for `jackstah` while both map
to the same accounts. Likewise, `cadenza-eng` contributes a distinct Codex
fallback but no distinct Claude fallback. `lf profile route show` should render
these aliases as shared rather than presenting them as additional capacity.

Failover starts a new provider child with the next profile. Credentials never
change underneath a live Claude or Codex process.

The basic CLI should read as the real model:

```sh
lf profile create jack --chrome-profile jack@loopflow.studio
lf profile account set jack claude jack@loopflow.studio
lf profile account set jack codex jack@loopflow.studio

lf profile route set \
  --default jackstah \
  --backup loopflow-eng \
  --backup cadenza-eng

lf profile route show
```

`lf profile route set` applies to the current repository unless `--repo` is
given. The update is atomic so a run never sees a partially changed order.

## Account lifecycle

Credential, billing, routing, and runtime health are different state:

```rust
struct ProviderAccount {
    id: ProviderAccountId,
    provider: Provider,
    login_email: EmailAddress,
    credential_state: CredentialState,
    routing_state: RoutingState,
    plan: Option<String>,
    paid_through: Option<Date>,
    health: AccountHealth,
}

enum RoutingState {
    Automatic,
    ExplicitOnly,
    Disabled,
}
```

`paid_through` and `plan` are operator-maintained facts unless the native CLI
reports them reliably. Loopflow does not scrape checkout pages or infer a
renewal date from token behavior. A canceled-but-still-paid account remains
`Automatic` through its recorded end date; afterward it becomes
`ExplicitOnly` unless the user changes it.

## Billing transition

Do not wait until the old subscription expires to create the new identity.
Prepare everything except the purchase:

1. Create real managed Google identities for `loopflow-eng@loopflow.studio`
   and `cadenza-eng@loopflow.studio`.
2. Route their verification and recovery mail to `jack@loopflow.studio` and
   verify delivery.
3. Create matching Chrome profiles, authenticate the free provider accounts,
   install/connect the provider extensions, and import their credentials into
   Loopflow.
4. Record each existing account's provider-specific billing date, cancel its
   renewal at least 24 hours beforehand, and keep routing through it for the
   remainder of the paid period.
5. Shortly before each provider expires, purchase the destination plan and
   validate one real provider launch through it.
6. Change the affected profile-to-provider mapping atomically. Claude and Codex
   can cut over on different days without changing the repository's profile
   route.
7. Leave `jackstah@gmail.com` registered as `ExplicitOnly` after downgrade, or
   map it to a deliberately named low-tier profile if it should remain a
   fallback.

Canceling early is safe for this schedule: both providers document continued
access through the paid billing period, and OpenAI documents that consumer
subscriptions cannot be transferred between accounts. The new subscription is
a separate purchase, not a transfer.

## Google account management

An email alias is insufficient for `loopflow-eng` or `cadenza-eng`: Google says
aliases are not Google Accounts and cannot sign in. They must be real managed
users if they are the Google OAuth identities.

The lowest-overhead setup to test is:

- enable Cloud Identity Free;
- turn off automatic paid Workspace license assignment for the engineering
  account organizational unit;
- create the two users as Cloud Identity identities;
- add recipient-address routing to `jack@loopflow.studio` for provider mail;
- confirm sign-in and mail delivery with `loopflow-eng` before repeating it for
  `cadenza-eng`.

Cloud Identity does not supply a Gmail mailbox. If Workspace routing cannot
deliver mail for those exact unlicensed identities in the current domain
configuration, assign paid Workspace seats or use a separate mail-routing
arrangement. Do not silently turn the addresses into aliases: that would lose
the distinct Google sign-ins.

Account creation, MFA, recovery, and checkout remain human-confirmed browser
steps. Loopflow can open the correct Chrome profile and drive ordinary OAuth
authorization after bootstrap.

## `lf ssh`

For the current repo, forward:

- the default and ordered backup profile records;
- every provider-account mapping referenced by those profiles;
- each unique credential exactly once, even when multiple profiles share it.

The remote side materializes process-lifetime credential homes, rewrites the
profile mappings to those homes, and removes them when the SSH process exits.
After a remote restart, the local controller re-forwards the same bundle. No
Chrome binding is forwarded: remote provider processes need credentials, while
browser control remains on the host that owns the Chrome profile.

## Migration from the current account-first router

1. Preserve existing provider credential homes and health rows.
2. Re-key `primary` and `loopflow` as provider accounts identified by their
   verified provider email.
3. Create Loopflow profiles separately and move Chrome-pairing metadata from
   provider-account homes to host-local profile bindings.
4. Create profile-provider mapping rows and one local repository route.
5. Migrate pinned sessions to carry both profile id and provider account id.
6. Remove provider-level `preferred`; preference now belongs only to the repo's
   ordered profile route.

## Proof

1. Two profiles mapped to one provider account share rate-limit health and are
   tried only once.
2. A repo selects its default profile, fails over in declared order, and pins
   the selected profile/account for the child lifetime.
3. Claude and Codex mappings can transition independently without changing the
   repo route.
4. `lf ssh` forwards a repo's candidate profiles and deduplicated credentials,
   then re-forwards them after restart without leaving credentials behind.
5. Chrome selection follows the host-local profile binding, while skills and
   provider configuration remain shared.

## Current service references

- Google Workspace: aliases are not Google Accounts:
  <https://support.google.com/a/answer/33327>
- Google Workspace: paid-license assignment:
  <https://support.google.com/a/answer/1727173>
- Google Workspace: recipient address maps:
  <https://support.google.com/a/answer/4524505>
- Anthropic: cancellation retains access through the billing period:
  <https://support.anthropic.com/en/articles/8325617-how-do-i-cancel-my-paid-claude-subscription>
- OpenAI: cancellation timing:
  <https://help.openai.com/en/articles/7232927-how-do-i-cancel-my-chatgpt-subscription>
- OpenAI: subscriptions cannot be transferred between accounts:
  <https://help.openai.com/en/articles/9135236-how-to-transfer-a-chatgpt-plus-or-chatgpt-pro-subscription-to-a-new-account>

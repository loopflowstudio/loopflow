# Profile routing and account lifecycle

## Current profile set

A Loopflow profile is the unit a repository routes through. Each profile binds
one host-local Chrome profile and one account per provider. Provider accounts
are reusable: multiple Loopflow profiles can deliberately point at the same
Claude or Codex account.

```text
repository
  default_profile: jack@loopflow.studio
  backup_profiles: [loopflow-eng@loopflow.studio, jackstah@gmail.com]
                         |
                         v
profile -------------------------------+
  chrome profile                        |
  claude account -> provider account <--+-- shared health and billing state
  codex account  -> provider account <--+
```

The current-phase mappings are:

| Loopflow profile | Claude account | Codex account |
|---|---|---|
| `jack@loopflow.studio` | `jack@loopflow.studio` | `jack@loopflow.studio` |
| `loopflow-eng@loopflow.studio` | `jackstah@gmail.com` | `loopflow-eng@loopflow.studio` |
| `jackstah@gmail.com` | `jackstah@gmail.com` | `jackstah@gmail.com` |

There is no `cadenza-eng`/`cadenza-dev` profile in this phase. The existing
`jackstah@gmail.com` profile stays available while its already-paid provider
plans are used and can later remain as a low-tier fallback after downgrade.
`loopflow-eng@loopflow.studio` is created and currently paid on Codex/OpenAI;
its Claude mapping intentionally reuses the personal account.

## Ownership

```rust
struct ProfileId(EmailAddress);

struct Profile {
    id: ProfileId,
}

struct ChromeProfileBinding {
    profile_id: ProfileId,
    host_id: HostId,
    chrome_directory: String,
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

The normalized Google/Chrome email is the profile identity exposed by the CLI.
A Chrome binding is host-local because `Profile 7` on one Mac means nothing on
another host. Provider accounts are keyed by provider plus normalized login
email in the user-facing model; credential-home paths use an opaque stable id
rather than putting email addresses in paths.

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
profile. If `loopflow-eng@loopflow.studio` and `jackstah@gmail.com` both resolve
Claude to `jackstah@gmail.com`, exhausting it removes both candidates for
Claude. The router cannot mistake two profiles for two pools.

This also means `jackstah@gmail.com` is not additional Claude capacity behind
`loopflow-eng@loopflow.studio` while both map to the same personal Claude
account. It is distinct Codex capacity. `lf profile route show` should render
shared provider mappings rather than presenting them as additional capacity.

Failover starts a new provider child with the next profile. Credentials never
change underneath a live Claude or Codex process.

The basic CLI should read as the real model:

```sh
lf profile create jack@loopflow.studio --chrome-profile jack@loopflow.studio
lf profile account set jack@loopflow.studio claude jack@loopflow.studio
lf profile account set jack@loopflow.studio codex jack@loopflow.studio

lf profile route set \
  --default jack@loopflow.studio \
  --backup loopflow-eng@loopflow.studio \
  --backup jackstah@gmail.com

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

1. Maintain the three real Google identities `jack@loopflow.studio`,
   `loopflow-eng@loopflow.studio`, and `jackstah@gmail.com`.
2. Route `loopflow-eng@loopflow.studio` verification and recovery mail to
   `jack@loopflow.studio` and verify delivery.
3. Create matching Chrome profiles, authenticate the provider accounts,
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
   keep that profile eligible if the low-tier account should remain a fallback.

Canceling early is safe for this schedule: both providers document continued
access through the paid billing period, and OpenAI documents that consumer
subscriptions cannot be transferred between accounts. The new subscription is
a separate purchase, not a transfer.

## Google account management

An email alias is insufficient for `loopflow-eng@loopflow.studio`: Google says
aliases are not Google Accounts and cannot sign in. It must be a real managed
user because it is a Google OAuth and Chrome profile identity.

The lowest-overhead setup to test is:

- enable Cloud Identity Free;
- turn off automatic paid Workspace license assignment for the engineering
  account organizational unit;
- create `loopflow-eng@loopflow.studio` as a Cloud Identity identity;
- add recipient-address routing to `jack@loopflow.studio` for provider mail;
- confirm sign-in and mail delivery before connecting provider OAuth.

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
2. Preserve opaque account ids such as `primary` and `loopflow`; record their
   verified provider login emails separately.
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

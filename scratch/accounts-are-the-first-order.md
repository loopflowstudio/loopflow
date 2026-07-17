# Accounts are the first-order primitive; profiles shrink to login ceremony

## Problem

An account is the thing that spends. A Chrome profile is a window you click
through once a month. Today's store has it backwards: profiles own per-provider
account mappings, repos route through an ordered list of *profiles*, and the
account — the subscription with the plan, the rate-limit window, the cooldown —
hangs off the end as a lookup result.

Three costs, all live in the store right now:

1. **Identity is assumed, not verified.** `ProfileId` *is* an email
   (`profile.rs:17`), so a venue and an identity are the same string. The store
   holds `loopflow-eng@loopflow.studio → claude/primary`, where `primary`'s real
   `login_email` is `jackstah@gmail.com`. Connect through that profile today and
   `resolve_profile_login` (`auth.rs:637-644`) takes the Claude branch, invents
   the login from the Chrome profile, and `register_managed_account` stamps
   `login_email = loopflow-eng@loopflow.studio` onto an account whose credential
   belongs to someone else. Silent, no error, wrong identity.

2. **Cross-provider bundling pretends the fleet is symmetric.** It isn't: 2
   claude accounts, 4 codex. One `repo_profile_routes.default_profile` plus
   ordered `repo_backup_profiles` forces both providers through one profile
   order. That order resolves to `[primary, loopflow]` for claude — the head of
   the list is the account whose `credential_state` is `missing` — and to
   `[jackstah-1066…, engineering, jack-42d…]` for codex. What you ordered is not
   what spends.

3. **An unrouted repo silently spends ambient credentials.** `Ok(None)` from
   `resolve_provider_account` (`provider_account.rs:589-594`) means three
   different things — not in a repo, repo not routed, no store — and every
   caller treats it as "do nothing", so the process inherits `~/.claude`. The
   difference between "deliberately unmanaged" and "I forgot to route this repo"
   is invisible, and forgetting costs money on the wrong account.

Who benefits: anyone running a fleet. This is Developer Efficiency's
"credential expiries pre-empt" KR and "avoidable human-in-the-loop repair steps
fall to zero" KR — a wrong-identity login is a repair step, and a route whose
head is a dead credential is an expiry nobody pre-empted.

## The demo

```
$ lf route show
claude   (default route — this repo has no route)
  1. loopflow      jack@loopflow.studio           max   connected   6% session, 31% weekly
  2. primary       jackstah@gmail.com             max   missing

codex    (default route — this repo has no route)
  1. jackstah-1066ea9c99d1  jackstah@gmail.com            connected   2% weekly
  2. engineering            loopflow-eng@loopflow.studio  connected   0% weekly
  3. jack-42d1021d3f2d      jack@loopflow.studio          connected   0% weekly
  4. manabot-eng            manabot-eng@loopflow.studio   connected  80% weekly  (strained)

$ lf auth connect claude primary
Profile 3 (expects jackstah@gmail.com): signed in as someone.else@gmail.com — skipped
Profile 8 (expects loopflow-eng@loopflow.studio): ok, opening Chrome
Claude reports loopflow-eng@loopflow.studio; account 'primary' is jackstah@gmail.com.
Refused: the login was discarded, 'primary' is unchanged.
No access profile could log in claude/primary. Sign Profile 3 in as
jackstah@gmail.com, or add a venue: lf auth access add claude primary --chrome-profile "Profile 9"
```

Two providers, different depths, no profile in the routing surface, and a
wrong-identity login that today succeeds silently now refuses with the reason
and the fix. Both halves run against the real store.

## Approach

**One arrow: account → ordered venues.** A venue is consulted only at a
ceremony, in order, and always verified.

### Schema (migration `0.11.027_accounts_first.sql`)

```sql
-- A venue: one Chrome profile, and who we EXPECT is signed in.
-- The expectation is not an identity. It is a claim to check.
CREATE TABLE access_profiles (
    profile_id       TEXT PRIMARY KEY,
    chrome_directory TEXT NOT NULL UNIQUE,
    expected_login   TEXT NOT NULL,
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
);

-- The inversion. An account owns the ordered list of venues that can log it in.
CREATE TABLE account_access_profiles (
    provider   TEXT NOT NULL,
    account_id TEXT NOT NULL,
    position   INTEGER NOT NULL CHECK (position >= 0),
    profile_id TEXT NOT NULL,
    PRIMARY KEY (provider, account_id, position),
    UNIQUE (provider, account_id, profile_id),
    FOREIGN KEY (provider, account_id)
        REFERENCES provider_accounts(provider, account_id) ON DELETE CASCADE,
    FOREIGN KEY (profile_id) REFERENCES access_profiles(profile_id) ON DELETE RESTRICT
);

-- One route shape, two scopes. What you order is what spends.
-- The store is host-local, so the machine default IS this store's default:
-- it needs no host key, and must not have one (see decision 2).
CREATE TABLE provider_routes (
    scope      TEXT NOT NULL CHECK (scope IN ('repo', 'default')),
    scope_id   TEXT NOT NULL,          -- RepoId when scope='repo', '' when 'default'
    provider   TEXT NOT NULL,
    position   INTEGER NOT NULL CHECK (position >= 0),
    account_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (scope, scope_id, provider, position),
    UNIQUE (scope, scope_id, provider, account_id),
    CHECK ((scope = 'default' AND scope_id = '')
        OR (scope = 'repo' AND scope_id <> '')),
    FOREIGN KEY (provider, account_id)
        REFERENCES provider_accounts(provider, account_id) ON DELETE RESTRICT
);

-- Sessions pin an account. A profile was never a run-time fact.
-- (rebuild provider_session_accounts without profile_id)

DROP TABLE repo_backup_profiles;
DROP TABLE repo_profile_routes;
DROP TABLE profile_provider_accounts;
DROP TABLE chrome_profile_bindings;
DROP TABLE profiles;
```

Rust: `RouteScope { Repo(RepoId), Default }` maps 1:1 to `(scope, scope_id)`.
`ProfileId` relaxes from `EmailAddress` to a printable slug; `AccessProfile
{ id, chrome_directory, expected_login: EmailAddress }` replaces `Profile` +
`ChromeProfileBinding` + `ProfileProviderAccount`. `HostId` is **deleted** — all
22 of its references are Chrome-binding-related, so nothing survives to use it.

### Selection ladder

```
LF_ACCOUNT (--account)  → explicit, ungated, verified live      [unchanged]
LF_FORWARDED_ACCOUNT_BUNDLE → ssh lease
provider_routes(Repo(repo_id), provider)
provider_routes(Default, provider)           ← new; catches unrouted repos
ambient                                       ← only when both routes are empty
```

Within a route: **order is intent, health is observation.** Health never
reorders intent; it only filters and demotes.

- **Exclude**: `credential_state != Connected`, `effective_routing_state !=
  Automatic`, or `cooldown_until > now`. (Today's rule, unchanged.)
- **Demote**: any window in `provider_account_limits` with `used_percent >= 90`
  and `resets_at` in the future sorts to the back, preserving relative order.
  Never excluded — a strained account still beats no account.
- Pinned session wins if still eligible. (Today's rule, unchanged.)

### Ceremony (`lf auth connect <provider> <account>`)

1. Read the account's ordered `account_access_profiles`.
2. For each venue in order: resolve `chrome_directory` in Chrome's `Local
   State`, require its signed-in `user_name == expected_login`. A venue whose
   directory no longer exists, or that is signed in as anyone else, → print the
   reason, fall through.
3. Open Chrome `--profile-directory`, log in to a tempdir home (today's
   pattern — a failed login cannot corrupt a working credential).
4. **Verify.** Require a provider-reported email:
   Claude → `claude auth status` (`email`), Codex → `id_token` claims. If the
   account has a `login_email`, the reported email must equal it or the login is
   discarded and the account is untouched. If it has none, adopt the reported
   email. **Never write an inferred login_email.**
5. Install into the account home; record `plan` from the provider's own report
   (`subscriptionType` / limits poll).
6. Exhausted list → one error naming every venue and why each was skipped, plus
   the two fixes (sign the venue in, or add a venue).

`--chrome-profile "Profile 9"` bootstraps: it names a Chrome directory directly,
verifies it the same way, and on success records the venue (`expected_login` =
the venue's actual Local State login) and appends it to the account's list. No
profile is ever invented to use an account.

`lf auth import` takes the same account-first shape and the same step 4.

### CLI

| Today | Target |
|---|---|
| `lf profile route set --default X --backup Y` | `lf route set claude <acct>…` (repo), `lf route default set codex <acct>…` (default) |
| `lf profile route show` | `lf route show` (repo route, or the default route with a note) |
| `lf auth connect claude --profile X` | `lf auth connect claude <account>` |
| `lf profile account set X claude <acct>` | `lf auth access set claude <acct> --profile A --profile B` (ordered), `lf auth access add/rm` |
| `lf profile create X --chrome-profile X` | `lf profile create --chrome-profile "Profile 8" [--as <name>] [--expects <email>]` |
| `lf profile list` | `lf profile list` — venue, expected login, **live** signed-in login, accounts listing it |

`lf auth accounts` and `lf usage` lead with the account and its `login_email`;
the venue list becomes a detail, reversing `account_label`'s profile-first
presentation (`usage.rs:171`).

### Forwarding

`ForwardedProfileBundle` → `ForwardedAccountBundle`: `repo_id`, per-provider
ordered account ids, account lifecycle rows, credentials. Venue data is dropped
entirely — ceremonies are local and a remote host has no Chrome. It is Rust→Rust
over an ssh env var with no Swift/Python mirror and no DTO fixture, so the
reshape is free.

### Migration, derived from the live store

| Target row | Derived from | Result on this machine |
|---|---|---|
| `access_profiles` | `profiles ⨝ chrome_profile_bindings`, `expected_login = profile_id` (today's id *is* the email), `host_id` dropped | 3 venues; `manabot-eng@loopflow.studio` has no binding → no venue, reported |
| `account_access_profiles` | `profile_provider_accounts` reversed; ordered by (venue matching the account's `login_email` first, then `updated_at`, then id) | `claude/primary → [jackstah@gmail.com, loopflow-eng@…]`; `codex/manabot-eng → []` |
| `provider_routes` (repo) | walk `[default] ++ backups` in position order, map per provider, dedup keep-first | claude `[primary, loopflow]`, codex `[jackstah-1066…, engineering, jack-42d…]` |
| `provider_routes` (default) | `routing_state='automatic'`, ordered `last_selected_at DESC NULLS LAST, account_id` | claude `[loopflow, primary]`, codex `[jackstah-1066…, engineering, jack-42d…, manabot-eng]` |

Dropping `host_id` makes `chrome_directory` unique store-wide. All 3 live
bindings are on one host, so this is a no-op today. If a store ever holds two
hosts' bindings for the same directory, the migration **fails loudly** rather
than picking one — choosing would be exactly the silent assumption this design
exists to delete.

The repo derivation is **behavior-preserving by construction**: today's runtime
already walks `[default] ++ backups`, maps each through
`profile_provider_accounts`, and dedups shared accounts
(`sqlite.rs:1069`, test `store/mod.rs:4969`). The migration computes that same
list once, at rest, instead of on every resolve.

`claude/primary` keeps `loopflow-eng@…` as a listed venue even though its
expected login can't be `primary`'s identity. That is deliberate: a venue is
"worth trying", not "known to work", and step 4 is what makes listing it safe.
It is also the demo.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Can Claude's identity be verified at all, or must it be assumed? The README says "Claude Code does not expose an account email after OAuth, so Loopflow uses the selected Chrome profile email." | **The README is stale.** Ran `CLAUDE_CONFIG_DIR=~/.lf/accounts/claude/loopflow claude auth status` (claude 2.1.210) → `{"loggedIn":true,"email":"jack@loopflow.studio","orgId":…,"subscriptionType":"max"}`. Per-home, and the parser already exists (`parse_claude_status_login`, `provider_auth/mod.rs:2215`). `connect` simply doesn't call it — it hardcodes `login = None` (`auth.rs:254`) and lets `resolve_profile_login` invent the email. | The whole design rests on this. "Verify, never assume" is achievable for both providers **today**, with no new machinery. Claude's assume-branch (`auth.rs:637-644`) is deleted, not preserved. If a future claude stops reporting `email`, connect hard-errors — a refusal beats a wrong write. Also frees `plan` and `orgId` from `lf auth set --plan`. |
| Does Codex need the venue for identity? | No. `codex_login_from_auth` (`provider_auth/mod.rs:2648`) decodes the `id_token` JWT `email` claim. Credential-intrinsic. (`codex login status` prints only "Logged in using ChatGPT" — the CLI is not the source.) | Identity evidence is provider-reported for both providers; the venue login is a pre-flight check only, never evidence about the account. |
| The directive: "account rows live in a shared store; homes (and possibly health) may need explicit HostId scoping." | **The premise is false.** The store is host-local (`~/.lf/loopflow.db`). The one cross-host path forwards to a *throwaway* store: `ssh.rs:339-347` exports `LF_FORWARDED_PROFILE_STORE="$LF_PROFILE_LEASE_DIR/router.db"` from `mktemp -d` with `trap … rm -rf` on EXIT, and hydration writes `home: None` into it (`provider_account.rs:855`). No account row ever reaches another host's durable store. | **No HostId scoping for `home` or health.** Scoping would be speculative generality against a shared store that doesn't exist. Safety comes from live verification, which homes already have (`retain_authenticated_account` flips a dead credential to `Missing` on resolve). Wrinkle resolved, not deferred. |
| Then is `host_id` on venues load-bearing? | **No — and it is worse than useless.** `HostId::local()` is `gethostname` and nothing more (`profile.rs:81`), so the stamp compares hostnames. It therefore cannot distinguish the case it exists for (this store was restored onto another Mac) from a case that is not a problem at all (this Mac was renamed), and it refuses identically in both. A stamp that yields a false refusal as often as a true one is a coin, not a guard. | **Delete the column.** `access_profiles` collapses `profiles` + `chrome_profile_bindings` into one table keyed by `profile_id` alone, with `chrome_directory` unique. The ceremony's real protection is step 4, which is the argument the rest of this doc already makes. A genuinely multi-host store would host-qualify the ids *then* — the same "don't build ahead of need" reasoning that killed the `home` scoping. |
| If `host_id` goes, can the machine default still be keyed by host? | **No, and this is the sharper half.** Keying `provider_routes(Host(host_id), …)` on `gethostname` would leave `HostId` with exactly one consumer — resurrected for a *new* purpose, on the same unstable primitive. Renaming the Mac would then silently drop the machine default, and every unrouted repo would fall straight back to ambient credentials: **problem 3, reintroduced by the mechanism meant to fix it.** | The store is host-local (row above), so "machine-wide default" *is* "this store's default". `RouteScope { Repo(RepoId), Default }`; the default scope carries no id (`scope_id = ''`, enforced by CHECK). `HostId` is deleted outright — all 22 references are Chrome-binding-related — which retires the `gethostname` instability rather than filing it. |
| Does `lf auth exec` (named in the directive) exist? | **No.** It landed in #1027 and was deleted in the #1029 branch (merged to main as `0dd1bf843`, in my base). `grep "fn exec_account"` → absent; `AuthCommand::Exec` → absent. Rationale, verbatim: *"auth is for ceremonies, not for running agents; interactive vendor use on a managed account goes through the same selection (lf -m codex --account <x> --tui)."* The installed release `lf auth --help` still lists it — the binary lags main. | Do **not** re-add it. The directive's "exec addresses accounts" is already satisfied by `lf --account` + `-m`, which is the stronger form of the same principle. Flagged as an incorrect assumption per the guiding constraint. |
| Is utilization usable as a demotion signal? | Yes. `provider_account_limits(provider, account_id, window, used_percent, resets_at, plan, source)` is populated live — `claude/loopflow` 6% session / 31% weekly, `codex/manabot-eng` **80% weekly**. But `utilization_percent` is written by `record_rate_limit` and read **only for display**; the sole gate is `cooldown_until` (`sqlite.rs:1128-1132`). | Demotion is new behavior, not a rewiring. Multiple windows per account → strained if **any** live window is ≥ 90%. Demote, never exclude. |
| How wide is the blast radius? | Contained. All five tables are touched **only** through `SqliteStore` — no raw SQL elsewhere. Consumers: `provider_account.rs`, `lf/commands/{profile,auth,usage,ssh}.rs`. No Swift or Python mirrors, no `tests/fixtures/dto/` entry (grep: zero hits). | One migration + one store file + the routing engine + 4 CLI files. Justifies doing the inversion in one PR instead of straddling both arrows. |
| Is migration ordinal `0.11.027` free? | Yes, **at each moment it was looked at.** `0.11.026` is the max on main; none of the 4 open PRs adds a migration (checked while drafting), and none of the 8 open PRs adds one either (re-checked at review — the count moved by three *while this doc was being written*). | Take `0.11.027`. **Re-scan at land, not at design:** `gh pr list` + `gh pr diff` for the migrations directory, and grep open diffs for the *tables*, not just the ordinal. Per wave memory a sibling merging any migration turns migration-check red on this branch — that failure is staleness, and rebasing is the whole fix. The ordinal is only free at the instant you look. |
| Does today's Chrome check actually prove the venue is signed in? | No — `resolve_local_chrome_profile` reads `user_name` from Chrome's on-disk `Local State` (`profile.rs:118-129`), a cached hint. `--profile-directory` is a request; an already-running Chrome may adopt the URL in another window. | Accepted, unchanged, and now *harmless*: the venue check is only a pre-flight that saves a doomed login. The load-bearing check is step 4, which reads the credential the ceremony actually produced. This is precisely why identity evidence must come from the provider, not the venue. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Keep `profile → account` and add a reverse index for the account view | Smallest diff; no migration of routes | Two arrows, one truth. The reverse index drifts, and the ordering (`repo_backup_profiles.position`) still lives on the wrong noun, so routing keeps pretending the fleet is symmetric. CLAUDE.md: keep one implementation. |
| Delete the profile row; store `chrome_directory` + `expected_login` inline per account | One fewer table; account is fully self-describing | The venue is genuinely shared — `Profile 3` backs both `claude/primary` and `codex/jackstah-1066…`. Inlining duplicates `expected_login` per account and lets the copies drift, re-creating "the expectation is not an identity" one level down. A shared venue is a real thing; give it a row. |
| Separate `repo_provider_routes` and `host_provider_routes` tables | Each key is honestly typed | Two near-identical tables and two copies of every store method, for one concept: an ordered account list at a scope. The tagged union (`scope`, `scope_id`) is explicit, not a lie — unlike a sentinel `repo_id = '<hostname>'` row, which was also considered and rejected for exactly that reason. |
| Default route as an ordinary route on a reserved repo id | No schema change | `repo_id` would hold a sentinel that is not a repo. The `scope` discriminator says the same thing honestly, and costs one column. |
| Machine default seeded from `credential_state='connected'` | Seeds only accounts that work right now | Bakes an observation into an intent. `claude/primary` is `missing` today and connected tomorrow; it would silently never enter the default. Seed from `routing_state='automatic'` (intent) and let health gate at selection. |
| Keep the assume-branch for Claude as a fallback when `email` is absent | Never breaks connect | It is the bug. It is what writes `loopflow-eng@…` onto `jackstah@gmail.com`'s account. A hard error is the requirement: "never a silent wrong-identity login." |
| Land the inversion in stages behind both arrows | Smaller PRs | Guarantees a dual-state window and compat shims for a change whose whole point is one arrow. CLAUDE.md: complete over incremental; no backwards compatibility for internal state. |

## Key decisions

1. **Identity evidence is provider-reported, always.** The venue's Chrome login
   gates *which window to open*; it never says who the account is. This is only
   possible because `claude auth status` reports `email` per home — the finding
   that unblocks the directive's "nothing may assume".
2. **`HostId` is deleted, not scoped.** The store is host-local — the "shared
   store" premise dies on the ephemeral lease store — so homes and health need
   no scoping, venues need no `host_id`, and the default route needs no host
   key. The primitive it would have rested on is `gethostname` with no stable
   machine id, which cannot tell "restored onto another Mac" from "renamed this
   Mac". Keying the *default route* on it would have silently dropped the
   machine default on a rename and dumped every unrouted repo back onto ambient
   credentials — reintroducing problem 3 through its own fix. One host-local
   store, one default route, no host key anywhere.
3. **Order is intent; health is observation.** Routes are declared and durable.
   Cooldown and credential state exclude; a strained window demotes. Neither
   ever rewrites the declared order.
4. **One `provider_routes` table with an explicit scope discriminator**, not two
   tables and not a sentinel row.
5. **The profile tables drop in the same PR**, against the directive's "shrink
   rather than drop until forwarding is rekeyed" — because forwarding is rekeyed
   in that same PR, so there is no window where both arrows exist.
6. **`lf auth exec` stays deleted.** The directive names it as an existing
   principle; main removed it deliberately and `lf --account` supersedes it.
7. **Venues that cannot possibly work are still migrated** (`claude/primary →
   loopflow-eng@…`). A venue list is candidates, not guarantees; verification is
   the contract that makes an honest import safe. **This row is not a bug and
   must not be "cleaned up" to zero** — the store is allowed to contain a venue
   whose expected identity contradicts the account's, because step 4 is what
   decides, and migrating only venues we *believe* will work would smuggle the
   assumption back in at import time. It is deliberately absent from Measure:
   its count moving is not the point, and a metric would invite the fix that
   re-breaks it.
8. **`ProfileId` keeps its current string values** (today's emails) while
   `expected_login` becomes a separate column. Renaming venues is an operator
   choice, not a migration; the point is that no code derives identity from the
   id any more. New venues default to naming themselves after their Chrome
   directory.

## Scope

**In scope (PR 2 — the inversion):** migration `0.11.027`; `access_profiles`,
`account_access_profiles`, `provider_routes`; `provider_session_accounts` loses
`profile_id`; `profile.rs` becomes the venue primitive; `HostId` deleted;
`resolve_provider_account` ladder incl. default route; verified
`connect`/`import`; `ForwardedAccountBundle`;
`lf route`, `lf profile`, `lf auth access`, `lf auth connect <account>`; README;
`scripts/demo_profile_routing.py` rewritten account-first.

**In scope (PR 3 — health policy):** utilization demotion from
`provider_account_limits`; `lf usage` / `lf auth accounts` account-first
presentation with the strained marker. Independent of the arrow: it changes
which eligible account wins, not who owns whom, and it is separately observable
(`manabot-eng` at 80% stops being picked before the provider refuses).

**Out of scope:** `RoutingState::Disabled` and `ExplicitOnly` remain indistinguishable at the
routing layer (both auto-excluded, both reachable by `--account`); `Disabled`
still has no enforcement point. Live Chrome session probing. Non-macOS venues.
Providers beyond Claude and Codex.

## Done when

```bash
# The arrow is gone from the schema.
sqlite3 ~/.lf/loopflow.db ".tables" | grep -Ev 'profile_provider_accounts|repo_profile_routes|repo_backup_profiles|chrome_profile_bindings'

# Routing addresses accounts, per provider, asymmetrically.
lf route show                        # 2 claude, 4 codex, no profile in sight
lf route set claude loopflow primary
lf route default set codex engineering jack-42d1021d3f2d
lf --account manabot-eng -m codex --tui   # escape hatch still above all of it

# Each account lists its venues, in order.
lf auth access set claude primary --profile jackstah@gmail.com --profile loopflow-eng@loopflow.studio
lf auth accounts claude              # account-first; venues are a detail

# The ceremony verifies and refuses.
lf auth connect claude primary       # walks venues in order; refuses on identity mismatch
lf auth connect claude primary --chrome-profile "Profile 9"   # bootstrap; invents no profile

# Nothing else regressed.
cargo test -p loopflow && cargo clippy -- -D warnings && cargo fmt --check
uv run pytest python/tests/
```

Tests that must exist (each pins a decision above, and each asserts on values
and on the refusal's message — not on `is_err()`, per wave memory):

- A venue whose Chrome login ≠ `expected_login` is skipped and the **next**
  venue is tried; the skip reason names both logins.
- A provider-reported email ≠ the account's `login_email` **refuses**, leaves
  `login_email` and the account home byte-identical, and leaves no tempdir.
- A provider reporting **no** email refuses (Claude's assume-branch is gone).
  Sabotage check: hardwire `parse_claude_status_login` to `None` — the test must
  fail.
- An exhausted venue list errors naming every venue and both fixes.
- Migration on a fixture matching the live store yields exactly the four derived
  results in the migration table above, and repo-route derivation reproduces
  today's `select_provider_profile` order for that repo (assert the account ids).
- An unrouted repo selects via the default route; with both routes empty it falls
  to ambient — and the three `Ok(None)` conditions are distinguishable.
- `--account` still bypasses route and health gating, and still verifies auth.
- A strained account (any window ≥ 90%, `resets_at` in the future) sorts last but
  is still selected when it is the only candidate. (PR 3)

## Measure

Baseline, from the live store on 2026-07-16:

- Accounts reachable only by inventing a profile: **1** (`codex/manabot-eng` —
  a profile with no Chrome binding). Target: 0.
- Route heads that are dead credentials: **1** (claude `[primary, loopflow]`,
  `primary.credential_state = missing`). Target: 0 after the default route seeds
  from `last_selected_at`, which puts `loopflow` first.
- Repos falling through to ambient credentials: **every repo except
  `loopflowstudio/loopflow`** (1 route row in the entire store). Target: 0 while a
  default route exists.
- Ceremony paths that infer an identity instead of reading one: **2**
  (`resolve_profile_login` Claude branch; `resolve_explicit_account`'s
  `login_email`→`ProfileId` back-derivation). Target: 0 — verified by deleting
  both and watching the suite stay green.

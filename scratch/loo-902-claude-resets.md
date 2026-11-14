# LOO-902: Check and redeem Claude weekly and Fable resets

## Problem

`lf account redeem-reset` spends banked resets for Codex only. Managed Claude
logins hold reset grants too (weekly and Fable), but `lf account` can't show
them and nothing in lf can spend one; the only route is Claude Code's own
at-limit dialog inside a live conversation.

## Demo

```
$ lf account claude=jack@
  …
  resets: 2 usable (live)
    weekly  full-reset-2611        1 of 1 left · usable now · clears weekly (70% used), weekly:fable (18% used) · ends 2026-12-10
    fable   launch-2610-fable-6mo  1 of 1 left · usable now · clears weekly:fable (18% used) · returns it to the weekly limit · ends 2027-03-09
    fable   launch-2610-fable      0 of 1 left · unavailable · clears weekly:fable (18% used) · returns it to the weekly limit · ends 2027-03-09
$ lf account redeem-reset claude=jack@ --scope fable
fable · launch-2610-fable-6mo · weekly 70% · weekly:fable 100%
Reset attempt 4b0e…; retry with --idempotency-key 4b0e… --reset-id launch-2610-fable-6mo
reset · fable · launch-2610-fable-6mo
  before  weekly 70% · weekly:fable 100%
  after   weekly 61% · weekly:fable 0%
$ lf --account claude=jack@ -m fable …      # routes to the login again
```

## What Claude offers (established 2026-11-14)

Source: Claude Code 2.1.325's bundled JavaScript (program name `cedar_ember`)
and one live read against a managed Max 20x login. No public documentation
exists; this is the same private surface Claude Code's at-limit dialog uses.

**Read.** `GET https://api.anthropic.com/api/oauth/usage?cedar_ember=1&skip_spend=1`
with the usual Bearer token and `anthropic-beta: oauth-2025-04-20`. Returned
200. The body carries `cedar_ember` (null without the query flag or when the
program doesn't apply):

```
eligible: bool, ineligible_reason: config_off|tier|seat|mobile|surface|cli_version|no_grant|tenure|other_experiment|unavailable|unknown|null
at_limit: bool, exhausted: [window], next_grant_id, weekly_resets_at, cooldown_until
grants[]: id (^[a-z0-9_-]{1,40}$), label, resets_total, resets_left, starts_at, ends_at,
          clears: [window], paused, usable_now, use_requires_limit,
          percent_used: {window: int}, blocking: [window],
          type: session|weekly|full, gives_back_weekly,
          unavailable_reason: cooldown|blocked_five_hour|blocked_seven_day|blocked_opus|blocked_sonnet|blocked_other|cannot_clear|needs_limit|unavailable|null
```

Windows: `five_hour`, `seven_day`, `seven_day_overage_included` (Fable's),
`seven_day_opus`, `seven_day_sonnet`, `seven_day_cowork`, `seven_day_omelette`,
`seven_day_oauth_apps`.

Live grants seen (login at seven_day 70%, Fable 18%, not at a limit):

| id | type | gives_back_weekly | clears | left/total | usable_now | ends_at |
|---|---|---|---|---|---|---|
| `full-reset-2611` | weekly | false | seven_day, seven_day_overage_included | 1/1 | true | 2026-12-10T22:00Z |
| `launch-2610-fable-6mo` | full | true | seven_day_overage_included | 1/1 | true | 2027-03-09T07:59Z |
| `launch-2610-fable` | full | true | seven_day_overage_included | 0/1 | false (`unavailable`) | 2027-03-09T07:59Z |
| `launch-2610-fable-12mo` | full | true | seven_day_overage_included | 0/1 | false (`unavailable`) | 2027-03-09T07:59Z |

`use_requires_limit` was false on all four and `usable_now` true at 18% used:
the provider lets a grant be spent before any limit is reached.
`next_grant_id` was `full-reset-2611`.

**Scope** is not a wire field. Claude Code derives it, and lf uses the same
rule so both name a grant the same way: `gives_back_weekly` → `fable`; else
`type` (`session`, `weekly`, `full`); else from `clears` (only `five_hour` →
session, none → weekly, both → full). The Task's "overall weekly" is `weekly`,
"Fable" is `fable`. A Fable reset "returns what Fable used to your weekly
limit" (Claude Code's copy).

**Spend.** `POST https://api.anthropic.com/api/organizations/{organizationUuid}/reset_rate_limits`,
JSON `{"program":"cedar_ember","grant_id":…,"request_id":…}`; `request_id`
matches `^[A-Za-z0-9_-]{1,64}$` (Claude Code sends a UUID and reuses it for
ten minutes while the same grant's claim is unsettled). Response:

```
result: reset|already_used|not_limited|cooldown|ineligible|unavailable
reason: (ineligible reasons)|paused|expired|unknown_grant|not_next_grant|grant_id_required|not_limited|already_used|cooldown|stamp_indeterminate|reset_unconfirmed|null
resets_left, cleared: [window], weekly_resets_at, cooldown_until
```

Claude Code treats HTTP 429 as rate limited, 401/403 as an auth error and
anything else as an error. It claims the first `usable_now`, unpaused,
unexpired grant of the chosen scope in the provider's list order.
The organization UUID is `organization.uuid` in `/api/oauth/profile`.

**Not established** (a real POST spends a grant, so none was sent): the exact
headers the POST needs beyond the read's, what a repeated `request_id`
returns, and whether `not_next_grant` fires when a scope other than
`next_grant_id`'s is claimed.

**Side finding.** The live body confirms the `limits[]` shape
`claude_windows` decodes (`kind`, `group`, `percent`, `resets_at`,
`scope.model.display_name`); the comment above it in
`rust/loopflow/src/subscription.rs` calling its fixture unconfirmed is out of
date. A second program, `juniper_tide` (`{program}` only, no grant), was null
on this login and is excluded.

## Starting architecture

- `subscription.rs`: `SubscriptionUsage.reset_credits: Option<RateLimitResetCredits>`
  is Codex's camelCase wire object passed straight through; `poll_claude`
  always sets it `None`. `redeem_codex_reset` validates identity, consumes,
  re-reads.
- `lf/commands/account.rs::redeem_reset` refuses any provider but Codex,
  prints the key before the request, then calls
  `store.reconcile_provider_account_capacity(account, revision, windows)`,
  which skips the write when a newer capacity revision exists and clears the
  account cooldown when its cause recovered. Its `bool` is ignored.
- Wire: `account.redeem_reset {provider, email, idempotency_key, credit_id}`
  (`lf/commands/write.rs`), `engine::account::{Redemption, UsageSnapshot}`,
  `AccountRow.reset_credits` in the account report. Fixtures:
  `rust/loopflow/tests/fixtures/dto/server/account_{redeem_reset,redeemed,report}.json`,
  `tests/fixtures/dto/{auth_status,server_schema}.json`. Swift mirrors only the
  request (`RegistryQuery.accountRedeemReset`, `ServerProtocolTests`).
- Status text: `account_status.rs` prints `banked resets:` for managed Codex
  rows only.
- Claude's test endpoints (`LF_TEST_CLAUDE_*`) exist only under `cfg(test)`,
  so Claude redemption tests live in the library test binary with the local
  `serve` stub already in `subscription.rs`, not in `tests/auth_tests.rs`.
- A poll names Fable's window `seven_day_fable`; Claude's stream and the
  grants name it `seven_day_overage_included`. The store already bridges the
  two (`sqlite.rs` `reconcile_provider_account_capacity`, `is_model_weekly_window`).

## Target

One provider-neutral reset record replaces the Codex pass-through; both
providers fill it and every reader shows the same thing.

```rust
pub struct Resets { pub usable: u64, pub resets: Option<Vec<Reset>>, pub unavailable: Option<String> }
pub struct Reset {
    pub id: String, pub scope: String,          // claude: session|weekly|fable|full; codex: its reset_type
    pub label: Option<String>, pub left: Option<u64>, pub total: Option<u64>,
    pub usable_now: bool, pub unavailable_reason: Option<String>,
    pub clears: Option<Vec<String>>,            // lf window names; null when unknown (Codex)
    pub expires_at: Option<i64>,
}
```

- `usable` is the provider's count for Codex (`availableCount`; details may be
  absent, the count stays authoritative) and the number of usable grants for
  Claude. `unavailable` carries Claude's `ineligible_reason` when
  `eligible` is false. A null `cedar_ember` block is `resets: None` at the
  account level: unknown, never zero.
- Live `lf account` and redemption read Claude usage with
  `?cedar_ember=1&skip_spend=1`, and so does every other Claude usage poll
  (one read; see direction 1 for the decode rule that makes this safe).
  `--cached` reports resets unknown.
- `lf account redeem-reset <provider>=<login> [--scope <scope>] [--reset-id <id>] [--idempotency-key <key>] [--json]`.
  `--reset-id` replaces `--credit-id` (wire `credit_id` → `reset_id`, plus
  `scope`). Claude picks the first usable grant of `--scope` in the provider's
  order. With no scope: one usable scope proceeds; several stop with the list
  and spend nothing. A retry passes the printed key and reset ID together and
  skips the usable check, since a completed spend leaves the grant unusable.
  For Codex, `--reset-id` is its credit ID and `--scope` filters credits by
  `reset_type` when the provider returned details; with neither, Codex chooses.
- Claude spend: fresh token → profile (identity + organization UUID) →
  `validate_identity` → usage read (before) → POST → usage read (after) →
  reconcile. The credential-unchanged checks that guard the usage read guard
  the POST.
- `Redemption` gains `scope`, `reset_id`, `reason`; `outcome` stays the
  provider's word. Claude outcomes: `reset` spent; `already_used`,
  `not_limited`, `cooldown`, `ineligible`, `unavailable` spent nothing and
  print `reason`. An unknown `result`, a timeout or a 5xx is unconfirmed:
  error naming the key and reset ID. 401 after one forced refresh is
  needs-login; 429 is reported as rate limited, unconfirmed.
- When `reconcile_provider_account_capacity` returns false the result says a
  newer observation was kept; the confirmed outcome still prints.
- No early-use refusal: naming the login and scope is the decision. The
  before-usage and the grant's `clears` percentages print before the request.

## Concept review contract (2026-11-14)

- Ordinary Claude polling retains windows when `cedar_ember` cannot decode,
  reports resets unknown, and warns once per malformed response. Redemption
  decodes strictly before any spend.
- `SubscriptionError::Unconfirmed { message, reset_id }` covers transport,
  429/5xx, unreadable replies and unknown outcomes after dispatch. The command
  formats retry flags from the selected ID and key. Pre-send refusals do not
  suggest retrying an attempt. Served errors distinguish `OutcomeUnknown`
  from `Invalid`; 401/403 remain login refusals with retry flags after selection.
- Fable status shows `clears weekly:fable (N% used)` and adds
  `returns it to the weekly limit`.
- `Reset` carries scope once; the private Claude decoder derives it from
  provider `gives_back_weekly`. Unknown `clears` is null for Codex; Claude
  supplies a list.
- Providers call `announce(&ResetAttempt)` after selection, before dispatch;
  only the command chooses terminal or served diagnostics.

Codex still lets the provider select when no scope is supplied. Claude stops
when usable resets span several scopes.

The six points above are in the working tree (second review, 2026-11-14).

### Next implement direction (unbuilt)

1. **A confirmed outcome is never reported as a refusal.** In
   `lf/commands/account.rs::redeem_reset`, a failed
   `reconcile_provider_account_capacity` after the provider answered returns
   an error today; `engine::account::redeem_reset` maps it to
   `ErrorCode::Invalid`, so a spent grant reads as "refused" and the
   before/after lines are lost. Treat it like the failed after-read row
   below: return the `Redemption` with the outcome, `after: null` and the
   save failure in `refresh_error`. Nothing was recorded, so
   `newer_observation_kept` stays false. Test: a `reset` outcome whose save
   fails still returns the outcome.
2. **One rule for the error tail, keyed on dispatch.** Before `announce`:
   `; nothing sent`. After it, for `NeedsLogin` and `Unavailable` alike:
   `; nothing spent; retry with …`. Today `Unavailable` after `announce`
   (credential changed before the POST, or a failed refresh after a 401 the
   POST already received) says "nothing sent" and drops the retry flags the
   attempt line just printed.
3. **Unconfirmed reads in order.** The message ends with the command to run:
   `<what happened>; outcome unconfirmed; retry with --idempotency-key …`.
   Today the retry flags sit before "outcome unconfirmed" because the
   command appends to `message` inside the variant's `Display`.

## Absent and error states

| Case | `lf account` | `redeem-reset` |
|---|---|---|
| `cedar_ember` null | `resets: not reported` | error, nothing sent |
| `eligible: false` | `resets: unavailable (<reason>)` | error with reason, nothing sent |
| grants, none usable | each line with its reason | error listing them, nothing sent |
| scope has no usable grant | — | error naming usable scopes, nothing sent |
| provider says `not_limited` / `cooldown` | — | outcome printed, exit 0 in `--json`, no capacity change claimed |
| reply lost | — | error with key + reset ID to retry |
| after-read fails, or its save fails | — | outcome kept, `after: null`, `refresh_error` |
| non-managed or API-key Claude login | no resets line | `no managed Claude login matches …` |

## Delete — do not maintain

- `RateLimitResetCredits`, `RateLimitResetCredit` and the `reset_credits`
  field everywhere (`subscription.rs`, `engine/account.rs`,
  `account_status.rs`, fixtures, schema). Codex decodes its wire object
  privately and maps into `Resets`.
- The Codex-only `ensure!` in `redeem_reset` and the `banked resets:` /
  `Provider::Codex` branch in status text.
- `--credit-id` / `credit_id` / Swift `creditId`.
- `Reset.gives_back_weekly`, empty Codex `clears`, subscription-owned
  `print_reset_attempt`, and string-based provider retry suffixes.

Must survive: Codex redemption behaviour and its tests in
`rust/loopflow/tests/auth_tests.rs` (reshaped to the new fields), the
key-before-request line, `served_reads.rs` spelling test.

## Forbidden outcomes

- A `claude_resets` field beside `reset_credits`, or a provider switch in a
  consumer deciding which to read.
- Clearing `cooldown_until` or limit rows directly after a spend; only an
  observed usage read changes capacity.
- A second recovery path for parked Sessions. Redemption ends in the same
  reconcile a refresh does; PR #1862 (LOO-811) keeps its open choices.
- Redemption from status, routing or a Graph launch.
- A test seam in production signatures; the reset URL override is `cfg(test)`
  like the other `LF_TEST_CLAUDE_*`.

## Overlap

- PR #1862 edits `lf/commands/account.rs`, `provider_account.rs`,
  `provider_account/activation.rs`, `docs/subscriptions.md`. Keep this
  change inside `redeem_reset`, `print_reset_usage` and the reset section of
  the doc; avoid `provider_account.rs`.
- LOO-901 (no PR found on 2026-11-14, rechecked after realign) owns rejection normalization: the
  free-text window rows `lf account claude --cached` shows today
  ("You've hit your Fable limit · resets Nov 16…") are its to fix. If the
  post-reset routing test below fails because of such a row, record it there
  rather than patching `reconcile_provider_account_capacity` here.

## Remaining verification

- Gate owns the suites, served-command equivalence and Swift request round-trip.
- Demo owns a real read of a managed Claude login and the first real redemption.
  The private POST's headers, repeated-key response and not_next_grant
  behaviour still need that real spend; no unattended attempt is planned.
- Publication and landing remain separate Graph steps.

## Checks (2026-11-14)

- `cargo nextest run -p loopflow --lib -E 'test(subscription::reset_tests::) | test(lf::commands::account::account_status::tests::reset_status_) | test(lf::commands::write::tests::) | test(server::schema::tests::)'` — 22 passed; includes malformed entitlement polling, typed served refusals/unconfirmed attempts, selected-ID retry flags, Fable percentages and null Codex clears. Nextest reported one leaky schema test; macOS linker warned about the large unwind section.
- `cargo nextest run -p loopflow --test integration -E 'test(auth_tests::banked_resets_require_explicit_redemption_and_reuse_attempt_identity) | test(documented_commands::)'` — 5 passed; Codex CLI regression took 72.618 s.
- `cargo clippy --all-targets -- -D warnings` — passed on the final Rust tree.
- `cargo fmt --all -- --check` — passed.
- `uv run python scripts/check_architecture.py` — passed.
- `cargo test -p loopflow --test served_reads server_operations_match_their_lf_commands` — gate: served-command equivalence.
- `scripts/test_desktop.sh --filter ServerProtocolTests` — CI: Swift request round-trip.
- `lf account claude` — demo: real read-only status.
- `lf account redeem-reset claude=<login> --scope fable` — demo: a real grant, only with a person's chosen login and scope.

## Done when

`lf account claude` lists a managed login's weekly and Fable resets with what
each clears and why one can't be used; `lf account redeem-reset claude=… --scope fable`
(or `weekly`) spends exactly that grant through Claude's endpoint, prints
before and after usage, and the next launch routes to the login; a retry with
the printed key never spends twice from lf's side; Codex works as before
through the same record.

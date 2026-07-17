# Account health demotes strained routes before refusal

## User-visible outcome

Declared routes remain operator intent, but an account whose provider-observed
limit window is nearly spent moves behind the route's unstrained accounts for
automatic selection:

```text
$ lf route show
codex  (default route — this repo has no route)
  1. manabot-eng          eng@example.com                  connected  demoted: weekly 80% used
  2. engineering          engineering@example.com          connected
```

The stored route is still `manabot-eng -> engineering`. Loopflow changes only
the effective walk for automatic, unpinned selection. A strained account is
demoted rather than excluded, so it still runs when it is the only eligible
candidate. Existing provider-session pins remain sticky when eligible, and an
explicit `--account` remains the operator escape hatch above route and health
policy.

This is PR 3 of W2-283. PR 2 already shipped the account-first ownership,
routing, ceremony, forwarding, and explicit-account model in #1053. This PR
does not revisit that inversion.

## Source of truth

Two existing persisted records answer different questions:

- `provider_routes` is the declared account order. Health never rewrites it.
- `provider_account_limits` is provider-observed window health. A row is
  actively strained when `used_percent >= STRAINED_UTILIZATION_PERCENT` and
  `resets_at > now`.

`STRAINED_UTILIZATION_PERCENT` is one policy constant beside the existing
cooldown policy in `provider_account.rs`:

```rust
pub(crate) const STRAINED_UTILIZATION_PERCENT: u8 = 75;
```

An account is strained when any of its windows is actively strained. A row
with no reset time cannot prove that its window is still active and does not
demote. A reset time at or before `now` is expired evidence and does not
demote. Missing limit rows mean no observed strain, not an error and not an
invented zero. The existing coarse `provider_accounts.utilization_percent`
may continue to record a provider event summary, but it is not authoritative
for reset-aware demotion or the strained label.

`active_account_strain` is the single classifier. Both orderings and the one
display marker call it, so there cannot be one threshold in display code and
another in the store.

The effective automatic order is:

1. keep candidates eligible under existing credential, routing-state, paid
   period, and cooldown rules;
2. if an eligible provider-session pin exists, select it, even when strained;
3. otherwise preserve declared relative order within two stable partitions:
   unstrained accounts first, strained accounts second;
4. select the first result, or report the existing per-account exclusion
   reasons when no eligible candidate remains.

## Forwarding carries the order, not the evidence

Health lives in the host's limit snapshot. Rather than shipping observations to
a remote that has no way to earn them, `local_forwarded_account_bundle` computes
the routed order once, before encoding, and forwards the answer. The remote
resolver walks the forwarded route in order and needs no health logic, no limit
hydration, and no wire field for observations.

This keeps the bundle a route-shaped payload: `selection`, `accounts`,
`credentials`. `AccountLimitRow` stays a store type with no serde derives.

## End-to-end proof

`ordinary_forwarded_route_demotes_strain_before_the_bundle_leaves_the_host`
builds a store with one Codex route declared `strained -> healthy`, an active
80% weekly row on `strained`, and real credentials for both. It calls
`local_forwarded_account_bundle`, asserts the encoded route reads
`healthy, strained`, then resolves that bundle against an empty throwaway
remote store and asserts the remote selects `healthy` and applies `healthy`'s
access token. The remote holds no limit rows, so the forwarded order is the
only thing that can carry the demotion; deleting the
`order_forwarded_routes_by_strain` call fails this test.

`automatic_selection_demotes_only_active_strain` covers the local selector: an
active 80% window demotes; the same account still wins when it is the only
candidate; and an under-threshold window, an expired reset, or a missing reset
all return declared order.

The pin test proves an eligible session pin still selects a strained account.

## Affected surfaces and consumers

- `SqliteStore::select_provider_account`: load limit rows inside the same
  selection transaction, apply the shared stable ordering, then retain the
  existing atomic `last_selected_at` and session-pin behavior.
- `local_forwarded_account_bundle`: read the limit snapshot once and order each
  routed provider's accounts before encoding.
- `lf route show`: bulk-read limit rows once and mark demoted accounts inline in
  the declared order. This is the only observability surface for demotion —
  demotion is a routing-order fact, so it belongs where route order is printed.
  `lf usage` already reports the windows behind the mark; it and
  `lf auth accounts` are untouched.
- README account-routing examples: show the marked output, define declared
  versus effective order, and state that strained means demoted, not disabled.

There are no Swift or external JSON DTO consumers. The forwarded bundle is an
internal Rust-to-Rust process-lifetime payload.

## Absent and error states

- No limit rows, `resets_at = NULL`, or an expired reset: ordinary declared
  order; never assume the account is healthy beyond saying no active strain
  was observed.
- Several active windows: any window at or above 75% strains the account. The
  marker names the highest-utilization qualifying window; ties use the stable
  stored window name so output is deterministic.
- Every eligible account strained: preserve declared order among them and
  select the first. There is no ambient fallback from a configured route.
- An account both strained and cooling: cooldown remains the stronger existing
  exclusion.
- Missing account row or existing credential/routing exclusion: preserve the
  current fail-closed reason. A limit-row read failure propagates as a store
  error; it must not silently disable demotion.

## Operational boundary

Runtime selection remains SQLite-only: no provider subprocess, browser, or
network refresh. Read all candidate windows in one query within the existing
immediate transaction; do not add N+1 account queries. `route show` bulk-reads
once. Ordering remains linear in candidates plus windows.

`lf usage` is still the only view here allowed to refresh provider evidence,
under its existing freshness and concurrency policy.

## Verification target

Focused checks:

```bash
cargo test -p loopflow --lib store::sqlite::tests::select_provider_account
cargo test -p loopflow --lib provider_account::account_first_tests
```

Final Rust gate for every touched test target:

```bash
cargo fmt --all -- --check
cargo clippy -p loopflow --all-targets -- -D warnings
cargo test -p loopflow
git diff --check
```

Sabotage controls:

- Set `STRAINED_UTILIZATION_PERCENT` above the fixture's 80%: the runtime and
  forwarded assertions must fail.
- Remove the `order_forwarded_routes_by_strain` call: the remote selection
  assertion must fail by choosing declared-first `strained`.

## Exclusions

- Refreshing provider limits during runtime selection or `route show`.
- A route explanation/preview command, or a second demotion marker on `usage`
  and `auth accounts`. One surface, where the order lives.
- Forwarding raw limit observations to a remote store.
- Per-provider or per-window thresholds, reset-horizon weighting, hysteresis,
  or a configuration knob. Add those only after real evidence shows one fixed
  threshold is wrong.
- Treating strain as disabled state, cooldown, or a route mutation.
- Changing explicit `--account`, connect/import ceremony, profile ownership,
  account homes, route schema, or SSH topology shipped in PR 2.
- Providers beyond the managed Claude and Codex account routes.

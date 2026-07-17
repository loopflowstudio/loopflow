# Account health demotes strained routes before refusal

## User-visible outcome

Declared routes remain operator intent, but an account whose provider-observed
limit window is nearly spent moves behind the route's unstrained accounts for
automatic selection:

```text
$ lf route show --explain
codex  (default route)
  1. manabot-eng  weekly 80%  demoted: strained (>= 75%, resets in 6d)
  2. engineering  weekly 0%   ready
  effective order: engineering, manabot-eng
  next selection: codex/engineering
```

The stored route is still `manabot-eng -> engineering`. Loopflow changes only
the effective walk for automatic, unpinned selection. A strained account is
demoted rather than excluded, so it still runs when it is the only eligible
candidate. Existing provider-session pins remain sticky when eligible, and an
explicit `--account` remains the operator escape hatch above route and health
policy.

`lf usage` and `lf auth accounts` mark the same active strained windows. Their
wording and `route show --explain` must agree with the account the runtime would
select.

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
const STRAINED_UTILIZATION_PERCENT: u8 = 75;
```

An account is strained when any of its windows is actively strained. A row
with no reset time cannot prove that its window is still active and does not
demote. A reset time at or before `now` is expired evidence and does not
demote. Missing limit rows mean no observed strain, not an error and not an
invented zero. The existing coarse `provider_accounts.utilization_percent`
may continue to record a provider event summary, but it is not authoritative
for reset-aware demotion or the strained label.

One pure classification/order helper consumes accounts, their persisted
windows, and an explicit `now`. Runtime selection and every explanation view
use that helper. There must not be one threshold in display code and another
ordering implementation in the store.

The effective automatic order is:

1. keep candidates eligible under existing credential, routing-state, paid
   period, and cooldown rules;
2. if an eligible provider-session pin exists, select it, even when strained;
3. otherwise preserve declared relative order within two stable partitions:
   unstrained accounts first, strained accounts second;
4. select the first result, or report the existing per-account exclusion
   reasons when no eligible candidate remains.

Selecting still updates `last_selected_at` and the session pin exactly once.
Previewing never does.

## End-to-end proof

Build a store with one Codex route declared as `strained -> healthy`:

- `strained` is connected and automatic, with a weekly row at 80% whose reset
  is in the future;
- `healthy` is connected and automatic, with a weekly row at 0%;
- both have usable credentials.

Then prove the same persisted facts across every consumer:

1. `lf route show --explain` prints the declared order, the 80% active-window
   reason, effective order `healthy, strained`, and `next selection:
   codex/healthy`, without changing either account's `last_selected_at`.
2. A real resolver call selects `healthy` and advances only its
   `last_selected_at`.
3. Encode that routed selection for SSH, hydrate an empty throwaway remote
   store, and resolve it there. The remote also selects `healthy`; forwarding
   cannot erase the evidence and revert to declared-first selection.
4. `lf usage --cached` and `lf auth accounts codex` both render `strained` for
   the first account from the same verdict.
5. Replace the route with only `strained`: automatic resolution selects it.
   Set an eligible session pin to `strained` in the two-account route: resume
   selects it. Resolve `--account strained`: explicit selection also selects
   it. These prove demotion did not become exclusion or override pinning.
6. Move the strained row's reset to `now` or the past: explanation and runtime
   return to declared order. Remove `resets_at`: neither invents strain.

The live smoke uses provider observations rather than a fitted fixture:

```bash
lf usage --refresh
sqlite3 ~/.lf/loopflow.db \
  "SELECT provider, account_id, window, used_percent, resets_at \
   FROM provider_account_limits ORDER BY used_percent DESC"

# Put an account with an active >=75% window before a healthy account for the
# same provider, preserving the prior route so it can be restored afterward.
lf route default set <provider> <strained> <healthy>
lf route show --explain
lf -m <provider> -- true
sqlite3 ~/.lf/loopflow.db \
  "SELECT account_id, last_selected_at FROM provider_accounts \
   WHERE provider='<provider>' ORDER BY last_selected_at DESC LIMIT 1"
```

On 2026-07-17 the store has two usable live proof candidates: Claude
`loopflow` at 99% of a future-reset session window and Codex `manabot-eng` at
80% of a future-reset weekly window. Re-read before the smoke; choose whatever
still meets the fixed policy. Do not move the threshold to chase the fleet.

## Affected surfaces and consumers

- `SqliteStore::select_provider_account`: load limit rows inside the same
  selection transaction, apply the shared stable ordering, then retain the
  existing atomic `last_selected_at` and session-pin behavior.
- A read-only selection preview: return declared/effective order, each
  account's eligibility or strain verdict, and the unpinned next selection.
  It shares classification with the mutating selector but performs no write.
- `lf route show --explain`: add the flag to `RouteCommand::Show`; use the
  preview for the repo route or its default fallback. Plain `route show`
  retains its compact output. The preview describes an unpinned next
  selection and says so when a session pin could change a resume.
- `lf usage`: keep its current poll/cache behavior. Mark windows/accounts
  strained from the persisted rows it already reads; `--cached` performs no
  provider call.
- `lf auth accounts`: bulk-read limit rows once and add the same strained
  marker to account-first output. Do not perform a provider poll or an
  account-by-account query.
- `ForwardedAccountBundle` routed selection: add a required top-level
  `limits: Vec<AccountLimitRow>` whose existing rows carry provider, account id,
  window, used percent, reset time, plan, original `observed_at`, and source.
  Carry rows only for the routed account snapshot and hydrate those exact
  values into the throwaway store before writing the route initialization
  marker. Do not reuse an upsert that stamps hydration time as observation
  time. Both ends ship together, so the internal wire field has no serde
  default or compatibility decoder. A restarted process in the same SSH lease
  keeps any newer health already recorded remotely, matching the current
  route-initialization rule.
- `ForwardedAccountBundle` pinned selection: selection still bypasses health.
  It need not hydrate limit windows to choose the concrete pin.
- README account-routing examples: show `--explain`, define declared versus
  effective order, and state that strained means demoted, not disabled.

There are no Swift or external JSON DTO consumers. The forwarded bundle is an
internal Rust-to-Rust process-lifetime payload.

## Absent and error states

- No limit rows, `resets_at = NULL`, or an expired reset: ordinary declared
  order; never assume the account is healthy beyond saying no active strain
  was observed.
- Several active windows: any window at or above 75% strains the account. The
  explanation names the highest-utilization qualifying window; ties use the
  stable stored window name so output is deterministic.
- Every eligible account strained: preserve declared order among them and
  select the first. There is no ambient fallback from a configured route.
- An account both strained and cooling: cooldown remains the stronger existing
  exclusion; explanation renders the exclusion rather than implying the
  account remains selectable at the back.
- Missing account row or existing credential/routing exclusion: preserve the
  current fail-closed reason. A limit-row read or decode failure propagates as
  a store error; it must not silently disable demotion.
- Routed forwarded bundle missing the required health collection or containing
  a limit row for an account outside its account snapshot: reject the bundle.
  Never fall back to ambient or native remote auth.
- `route show --explain` with no configured repo or default route: retain the
  existing ambient message and add no fictional effective order.

## Operational boundary

Runtime selection remains SQLite-only: no provider subprocess, browser, or
network refresh. Read all candidate windows in one query within the existing
immediate transaction; do not add N+1 account queries. CLI views also bulk-read
once. The route lengths are fleet-sized, but ordering remains linear in
candidates plus windows.

`lf usage` is still the only view here allowed to refresh provider evidence,
under its existing freshness and concurrency policy. `route show --explain`
and `auth accounts` must be fast, read-only views of persisted evidence.

Forwarded hydration writes the snapshot only while initializing a lease. The
route stays the last initialization marker so a provider restart cannot
overwrite health learned inside the lease.

## Verification target

Focused checks:

```bash
cargo test -p loopflow --lib store::sqlite::tests::select_provider_account
cargo test -p loopflow --lib provider_account::account_first_tests
cargo test -p loopflow --lib lf::commands::profile::tests
cargo test -p loopflow --lib lf::commands::auth::account_first_tests
cargo test -p loopflow --lib lf::commands::usage::tests
```

Final Rust gate for every touched test target:

```bash
cargo fmt --all -- --check
cargo clippy -p loopflow --all-targets -- -D warnings
cargo test -p loopflow
git diff --check
```

Sabotage controls:

- Set `STRAINED_UTILIZATION_PERCENT` above the fixture's 80%: the runtime,
  preview, forwarded, usage, and auth assertions must fail.
- Remove forwarded window hydration: the remote selection assertion must fail
  by choosing declared-first `strained`.
- Make preview call the mutating selector: the unchanged `last_selected_at`
  assertion must fail.

## Exclusions

- Refreshing provider limits during runtime selection, route explanation, or
  `auth accounts`.
- Predicting a particular existing session's pinned selection from
  `route show --explain`; the preview is explicitly unpinned.
- Per-provider or per-window thresholds, reset-horizon weighting, hysteresis,
  or a configuration knob. Add those only after real evidence shows one fixed
  threshold is wrong.
- Treating strain as disabled state, cooldown, or a route mutation.
- Changing explicit `--account`, connect/import ceremony, profile ownership,
  account homes, route schema, or SSH topology shipped in PR 2.
- Providers beyond the managed Claude and Codex account routes.

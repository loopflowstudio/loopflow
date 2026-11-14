# LOO-900: reconcile Claude capacity

## Outcome and demo

Refreshing a managed Claude login makes routing and account status agree about
which models it can run. A Fable limit does not stop Opus, Sonnet or Haiku.
A genuine account-wide limit still stops all of them. Refresh never erases a
rejection that arrived while the request was in flight.

Demo, reproduced headlessly with isolated credentials, a fixture usage server
and a provider stand-in: seed the two accounts below with their old rejections,
run `lf account claude --json`, then resolve fresh and resumed launches for
`claude:fable` and `claude:sonnet`. Fable can use the first login, Sonnet can use
either, and the second login's exhausted Fable allocation remains unavailable.
`lf account next --json` explains its existing model-unspecified decision;
`--agent` does not turn that command into a model-specific probe.

## Incident observations

The Task's supplied incident record reports the following around
2026-11-14 15:18 UTC; these are historical observations, not a new live probe:

| Login | Poll weekly used | Poll Fable used | observed_at | Weekly reset |
| --- | ---: | ---: | ---: | --- |
| jack@loopflow.studio | 66% | 11% | 1794669477 | 2026-11-17T05:59:59Z |
| jackstah@gmail.com | 65% | 100% | 1794669478 | 2026-11-18T12:00:00Z |

Both credential verifications succeeded. Older stream rows named
`seven_day_overage_included`, model `fable`, 100% used, with `limited_until`
1794895205 and 1795003205 respectively. Both account summaries had
`cooldown_until: null`, while `account next` still reported cooling. Reopening
both Fable and non-Fable conversations encountered cooldown errors.

## Findings that determine the design

- `provider_account.rs::account_held_until` combines the account cooldown with
  every applicable window's `limited_until`. `limit_affects(scope, None)` is
  deliberately conservative. `save_polled_usage` clears only the account
  cooldown; the poll's `seven_day_fable` key cannot replace a stream row named
  `seven_day_overage_included`. This explains the first login remaining blocked
  even after its poll found Fable capacity.
- `store/sqlite.rs::upsert_provider_account_limits` keys on
  `(provider, account_id, window)`, overwrites on arrival, timestamps at save
  time in seconds, and retains omitted windows. `save_polled_usage` writes
  windows and clears cooldown in separate transactions, using a previously
  loaded account. A delayed poll can overwrite a newer rejection even on a
  matching key. Wall-clock timestamps alone cannot order same-second writes.
- `harness/claude_decode.rs::rate_limit_signal` uses
  `rate_type.contains("overage")` to classify credits. That conflates an
  included per-model allocation with paid-credit exhaustion.
  Anthropic's [SDK changelog, 0.3.191](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
  describes `seven_day_overage_included` as a per-model weekly limit and adds
  `model_scoped` usage windows. Checked 2026-11-14. This establishes the bucket
  category; it does not establish that every future such event means Fable.
- Anthropic's [usage credit documentation](https://support.claude.com/en/articles/12429409-manage-usage-credits-for-paid-claude-plans)
  distinguishes included limits from paid usage and describes a separate Fable
  weekly allocation. Its [reset documentation](https://support.claude.com/en/articles/17007452-what-is-a-limit-reset)
  says a Fable reset restores Fable weekly usage without restoring other models'
  usage. Checked 2026-11-14. Overall weekly headroom alone cannot refute a
  Fable rejection, and included headroom cannot refute a paid-credit rejection.
- `subscription.rs::claude_windows` reads a `limits[]` array and maps weekly
  model display names to `seven_day_<model>`. Its comment correctly calls the
  existing fixture synthetic. The incident establishes decoded observations,
  not a captured raw response. Keep that distinction in new fixture names;
  do not claim a newly verified wire schema. Normalize recognized family
  labels consistently with launch models; unknown labels stay distinct.
- `subscription.rs::poll_account` sets exhausted windows' holds through reset
  plus five seconds. Keep that grace. Missing resets must remain unknown,
  rather than inventing a week-long deadline. Existing rejection fallback is
  15 minutes plus grace.
- Consumers include ordinary/exact/shared-home launch resolution,
  `LaunchCapacity` for open conversations, store route queries, urgency
  ranking, `hold::ready/end_when_full`, account reports, and `profile::next`.
  A stale 100% alias also distorts urgency, so merely ignoring its hold in one
  routing function is insufficient. Explicit account selection's current
  routing-policy bypass remains unchanged.
- `docs/subscriptions.md` promises that a fresh read repairs a wrong cooldown;
  the implementation repairs only one representation. Its description of
  `next` also sounds model-specific although `profile::next_launch` supplies
  no model. Clarify both descriptions.

## Chosen architecture

Keep capacity in `provider_accounts` and `provider_account_limits`; do not add
a parallel effective-capacity cache. Reconcile observations at the store write
boundary so every reader receives the same answer. Keep wire DTO shapes.

### One identity for a known bucket

Normalize known included per-model stream buckets to the poll's canonical
`seven_day_<family>` identity before persistence. For
`seven_day_overage_included`, use a known reported model, otherwise a known
launched family; an omitted launch model uses the configured Claude default
family, as the existing recorder does. Do not hardcode all overage-included
events to Fable. Keep paid-credit and unknown buckets separate. Replace the
decoder's substring classification with exact supported bucket meanings;
retain the existing stated-credit behavior independently.

Existing model-scoped alias rows must stop participating as separate capacity.
In the reconciliation transaction, a poll containing that same family's
weekly window supersedes the older alias, whether it reports room or 100%.
Remove the superseded alias from the latest-observation table; retain unrelated
and unscoped unknown rows. No wholesale deletion of omitted windows. Until a
matching observation arrives, existing scoped aliases continue limiting their
stored model. Do not infer an unknown alias's scope from aggregate headroom.
When comparing pre-existing alias and canonical rows before a fresh matching
poll, keep the newer observation; equal historical timestamps retain the more
restrictive result until refresh. Future normalized writes use the revision
ordering below, so they cannot produce this alias ambiguity again.

### Atomic freshness

Add one internal integer capacity revision column to `provider_accounts`, via
one additive migration. It is store synchronization metadata, not another
capacity record and not a field on public `ProviderAccount` or wire DTOs.
Every capacity write (health, windows, manual clear, invalidation that changes
cooldown) advances it transactionally. Record a stream signal's health and
windows together rather than as two visible partial writes.

Read the revision immediately before polling, outside any write transaction.
Pass that internal token to the atomic reconciliation write. Commit only if
the revision is unchanged, then advance it. A changed revision means the whole
capacity result was superseded: save neither its windows nor its cooldown
clear, and return that outcome. This handles same-second rejections, concurrent
polls and delayed responses without changing timestamp DTOs. It may discard a
valid reading after an unrelated capacity update on the same account; a later
refresh can retry normally. Do not add an automatic retry loop or hold SQLite's
write lock during provider I/O.

The transaction installs returned windows, removes only superseded known
aliases, and clears an old account cooldown only when a recognized complete
account-wide reading refutes its recorded cause. For known global weekly or
session causes require the corresponding returned window with room; for a
known model cause require that model's window, then replace the broad hold with
the scoped result. For the historical unqualified rejection with an unknown
cause, retain the hold until expiry or specific recovery; aggregate capacity
does not establish what failed. Failed, empty, missing relevant, or malformed
observations do not establish recovery. Never clear paid-credit exhaustion
from included-quota percentages.

Update all polling callers: verified report, first read after connect, poll
after stated rejection, and reset refresh. The shared API's Codex callers must
also supply correct tokens; reset handling must not install its older `before`
snapshot over an available `after` snapshot. Retain identity/credential
replacement checks. Reports reload committed account state and windows after
reconciliation; superseded results do not populate `verified_windows` or claim
fresh capacity. Authentication can still be accepted independently.

Keep existing conservative semantics for model-unspecified `next`; explain
scoped blockers in its reason and docs. A model-specific launch and resumed
conversation must carry their actual model to the common eligibility reader.
Follow those callers when fixing the reported non-Fable reopen failure rather
than weakening `limit_affects(None)` globally. Human-readable account details
show the scoped holds; `cooldown_until: null` continues to mean no account-wide
cooldown, not unrestricted capacity on every model.

## Alternatives and failure modes

- Clear all cooldowns on every successful poll: rejected; authentication,
  aggregate quota and per-model capacity answer different questions.
- Match aliases or timestamp precedence only in launch eligibility: rejected;
  status, urgency and held-account release would keep contradictory records.
- Compare second-resolution timestamps or completion order: rejected; a
  response begun before a rejection can arrive afterward in the same second.
- Per-bucket generations would admit more concurrent observations, but add
  bookkeeping across aliases and broad cooldowns. Choose one revision per
  account for this change; avoid speculative machinery.

Success means refresh repairs both routing and its explanation, including
already-open conversations, without manual DB changes. Failure would be silently
erasing a valid rejection because some unrelated percentage is below 100, or
retaining a stale alias that still affects account ranking. Tests must catch
both. Another failure is a recorder path that forgets the revision increment;
keep capacity mutations behind the store operations and exercise concurrent
writers with deterministic barriers.

## Concept review — 2026-11-14

Outcome: iterate to implement. No open product choice. The existing requirement
that genuine account-wide exhaustion stops every model determines the remaining
correction: a bucket's known scope takes precedence over incidental model
metadata. A reported model supplies the family for an included per-model bucket
that does not name one; it cannot narrow `seven_day` or `five_hour`, or contradict
a family already named by `seven_day_<family>`.

The current decoder takes `rate_limit_info.model` before interpreting the bucket.
Counterexample: `rate_limit_type: seven_day`, `model: fable`, rejected at 100%
becomes a Fable-only window with no account cooldown; Sonnet can launch despite
aggregate exhaustion. A second counterexample, `seven_day_fable` with
`model: sonnet`, stores a Fable key with Sonnet scope. These are synthetic review
inputs, not captured provider responses. Interpret known global and named-family
buckets first, use reported/launch family only where bucket semantics require it,
and keep unknown buckets distinct. No additional capacity record or public API
is needed. Keep the existing model-unspecified behavior of `next`.

Next implement direction: correct scope precedence at decoding/normalization and
add decoder-to-store routing regressions for both counterexamples. Show that a
Fable-only poll cannot recover the global rejection, a matching global reading
with room can, and the conflicting-label Fable bucket affects Fable alone.
Retain the existing in-flight refresh winner and report behavior. The review
changed only this design and `scratch/concept-review.html`; no checks were rerun
or production files changed during this step.

## Implementation and remaining delivery

The implementation follows the single-store revision design. Fresh and resumed
launches already pass their actual model through the common resolver; their
regression fixture exercises that path without changing conservative `next`.
The account fixture calls the real report/output handlers with synthetic usage
responses and isolated credentials; it starts no provider process.

A stated, unscoped “weekly limit” names the known aggregate weekly cause.
An unqualified “usage limit” remains unknown. Known model statements use the
canonical weekly key; stated paid-credit failures keep their separate key.
Missing reset times retain an existing rejection deadline while leaving
`resets_at` unknown. Negative percentages, malformed scopes and invalid reset
strings cannot establish recovery; percentages above 100 saturate at exhaustion.

### Delete — do not maintain

- The old unconditional poll upsert plus separate cooldown clear inside
  `provider_account::save_polled_usage` is replaced by atomic reconciliation.
- The recorder's separate health/window writes are replaced by one capacity write.
- The decoder's `contains("overage")` classification and its
  `a_rejected_overage_is_a_credits_limit_naming_no_model` assertion are removed.
- Known aliases are removed only by a matching model refresh. The store's legacy
  read comparison handles historical competing rows until that refresh.
- General window writes remain for other callers and advance the same revision.

No live account records were edited. Publication and the gate's broader suites
remain with the following Graph steps.

## Acceptance and unattended checks

Create `claude_usage_reconciliation_` named tests in existing library test
modules, including a CLI-dispatch fixture using the real account handlers and
fixture endpoint. Detached stand-ins must re-execute the test binary because
`LF_TEST_CLAUDE_*` endpoints are test-only; never launch the installed provider.
Use isolated HOME/LF_HOME and repository routes, await fixture completion, and
coordinate races with barriers, never sleeps. Add an upgrade test starting at
the merged schema frontier for the additive revision column.

The fixture drives decoder → persisted rejection → poll → reconciliation →
account JSON/text → `next` → fresh and resumed model-specific resolution:

- Seed both incident logins. First Fable becomes available after its matching
  11% poll; second Fable stays blocked at 100%; both allow non-Fable models when
  account-wide windows have room. Assert hold deadlines, ranking and account
  isolation as well as selected login. Re-run reads to establish recovery lasts.
- Real global exhaustion blocks every model. A model-only poll cannot clear it;
  a newer matching global window with room can. Fable-only exhaustion never
  releases a held account as globally full.
- Rejection after a poll, and rejection during an in-flight poll in the same
  second, both win. Two polls completing out of order cannot roll capacity back.
  A warning on a different bucket cannot clear an active rejection.
- Unknown/paid-credit buckets, absent scoped windows, empty response, malformed
  windows, failed poll and credential replacement retain appropriate holds.
  A conflicting legacy alias cannot override newer canonical capacity.
- At reset plus grace the applicable hold expires; a manual provider reset
  observed by a matching fresh poll restores capacity before natural expiry.
  No reset time uses the existing bounded rejection fallback; test this without
  turning missing usage into zero usage.
- `next` remains model-unspecified and reports scoped reasons honestly. Status
  and routing read the committed winner when a refresh loses a race. Codex
  observations, connect and reset refresh keep their current outcomes.

Commands below run with inherited `LF_*` removed and stdin closed, as required
by `scripts/testing.md`; account tests run from an empty temporary directory
with isolated HOME when invoking the built test binary directly.

- `cargo test -p loopflow --lib claude_usage_reconciliation_` — implementation
  focused check — passed, 11 tests, including the account-handler race, upgrade
  and indexed 10,000-account lookup checks.
- `uv run python scripts/test.py --reuse-passing --rust-filter 'test(claude_usage_reconciliation_) | test(provider_account::) | test(subscription::) | test(harness::claude_decode::) | test(lf::commands::account) | test(lf::commands::profile::) | test(store::sqlite::account) | test(documented_commands::)'`
  — gate runs once; expected affected routing, store, status, parser and docs
  checks pass. Confirm selectors against the suite list after implementation.
- `cargo fmt --check` — passed.
- `cargo clippy --all-targets -- -D warnings` — passed; CI owns the full suite.
- Built library test binary, isolated cwd/HOME/LF_HOME: matching cooldown recovery,
  stated weekly recovery, Fable scope and unknown percentages — four passed.
- `EXPLAIN QUERY PLAN` for new revision/reconciliation lookups on an isolated
  10,000-account fixture — implementation checks indexed provider/account and
  window lookups; no scan across unrelated account observations.
- `uv run python scripts/check_migrations.py` — passed; one additive migration.
- `uv run python scripts/check_architecture.py` — passed.
- `lf context` and `git diff --check` — within targets and clean.

Done when a refresh restores usable models, retains genuine scoped/global
limits, survives newer rejections and explains the same committed capacity
through CLI status, routing and conversation reopen. The two-account fixture
must pass without any manual cooldown clearing.

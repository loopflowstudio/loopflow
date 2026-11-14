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

## Facts that determine the design

At checkout base `53a62ddfa7113c61314aeaf885a1f137b6b62c60`,
`provider_account::save_polled_usage` upserted poll windows and cleared the
account cooldown separately. A poll's `seven_day_fable` could not replace a
stream's `seven_day_overage_included`; second-resolution timestamps could not
order an in-flight poll against a newer rejection.

- The original decoder conflated included per-model allocation and paid-credit
  exhaustion with `rate_type.contains("overage")`. Anthropic's [SDK changelog, 0.3.191](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
  describes `seven_day_overage_included` as a per-model weekly limit and adds
  `model_scoped` usage windows. Checked 2026-11-14. This establishes the bucket
  category; it does not establish that every future such event means Fable.
- Anthropic's [usage credit documentation](https://support.claude.com/en/articles/12429409-manage-usage-credits-for-paid-claude-plans)
  distinguishes included limits from paid usage and describes a separate Fable
  weekly allocation. Its [reset documentation](https://support.claude.com/en/articles/17007452-what-is-a-limit-reset)
  says a Fable reset restores Fable weekly usage without restoring other models'
  usage. Checked 2026-11-14. Overall weekly headroom alone cannot refute a
  Fable rejection, and included headroom cannot refute a paid-credit rejection.
- `subscription::claude_windows` consumes a synthetic `limits[]` fixture.
  The incident supplies decoded observations, not a captured raw response;
  it does not establish a newly verified wire schema.
- Routing, account status, urgency, held-account release and open conversations
  read the same persisted capacity. Fixing launch eligibility alone would leave
  stale aliases distorting ranking and explanations. Fresh and resumed launches
  already pass their model through the common resolver.

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

## Scope precedence

A known bucket determines scope before incidental model metadata.
`harness/claude_decode.rs::rate_limit_signal` uses the reported model only for
`seven_day_overage_included`. Synthetic review counterexamples (not captured
provider responses): `seven_day` with `model: fable` must still block Sonnet;
`seven_day_fable` with `model: sonnet` must still hold Fable alone. The decoder
regression follows both through recording and refresh.

The earlier review walkthrough is recoverable from
`f004f5a1ab54075d7f6f5900ee70cf8d9b3cdbd6:scratch/concept-review.html`;
its proposed correction is implemented.

## Implementation constraints

A stated, unscoped “weekly limit” names the aggregate weekly cause; an
unqualified “usage limit” remains unknown. Known model statements use the
canonical weekly key; stated paid-credit failures keep their separate key.
Missing reset times retain an existing rejection deadline while leaving
`resets_at` unknown. Negative percentages, malformed scopes and invalid reset
strings cannot establish recovery; percentages above 100 saturate at exhaustion.

No automatic retry, public revision DTO, separate capacity cache, or per-bucket
generation. One account revision may discard an otherwise valid poll after an
unrelated capacity write; a later refresh can retry normally. Do not weaken
`limit_affects(None)` or change explicit account selection's routing bypass.

### Delete — do not maintain

Removed:

- Unconditional poll upsert plus separate cooldown clear. All four poll callers
  now use `Store::reconcile_provider_account_capacity` directly; the forwarding
  `provider_account::save_polled_usage` wrapper is deleted.
- Separate recorder health/window writes; one atomic capacity write survives.
- Decoder `contains("overage")` classification and its obsolete credit assertion.
- The completed concept review's obsolete implementation walkthrough.
- Single-deadline cooldown explanations that hide a longer model hold behind
  an account-wide label; shared per-scope explanations survive.

Retained deliberately:

- Legacy alias comparison until a matching model refresh deletes that alias;
  unknown, unrelated and omitted windows remain.
- General health/window writes for other callers; both advance the revision.

The store shares one canonical model-weekly predicate for recovery, alias
removal and legacy comparison. Legacy selection consumes winning rows without
cloning them; stream normalization runs in one pass. Recovery tests remain on
the surviving store path.

## Checks and remaining delivery

No live account records were edited. Publication and gate's broader suites
remain with subsequent Graph steps. The existing synthetic account-handler
fixture covers both incident logins, JSON/text reports, `next`, fresh/resumed
routing and a rejection arriving during refresh; it launches no provider.

Earlier implementation run results (before compression):

- `cargo test -p loopflow --lib claude_usage_reconciliation_` — passed, 12 tests,
  including account-handler race, migration upgrade and indexed 10,000-account
  lookups.
- Built library test binary with isolated cwd/HOME/LF_HOME — four additional
  cooldown/stated-weekly/Fable-scope/unknown-percentage checks passed.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `uv run python scripts/check_migrations.py`,
  `uv run python scripts/check_architecture.py`, `git diff --check` — passed.

Compression checks (inherited `LF_*` unset; test/clippy stdin closed):

- `cargo test -p loopflow --lib claude_usage_reconciliation_` — passed, 12 tests;
  test-binary linker warned that `__eh_frame` exceeds compact-unwind offsets.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `git diff --check` — passed.
- `lf context` — 2,500 tokens / 11,868 bytes before these check results;
  within both scratch targets.

Gate selects a few named high-risk tests; CI runs suites. Keep race ordering,
matching-cause recovery, unknown/paid-credit retention, reset grace and missing
reset behavior covered. A refresh must restore usable models without erasing
newer rejection or making account status disagree with routing.

QA follow-up uses shared per-scope descriptions: `next` gives account-wide
and Fable holds separate deadlines. `--clear-cooldown` names remaining window
holds; an already-null cooldown says no active account-wide cooldown. Eligibility
and stored windows keep their existing semantics.

Concept review, 2026-11-14: no further design change or open product choice.
The overlapping-hold correction above is present in the working tree and fits
the existing model: one login has independently scoped capacity holds, and
clearing its account cooldown leaves observed windows intact. The review page
at `scratch/concept-review.html` shows the failure and recovery interaction.
Next: gate the implemented correction; no additional implement direction.

- `cargo test -p loopflow --lib claude_usage_reconciliation_explains_overlapping_and_cleared_holds` — passed (1); linker repeated the compact-unwind size warning above. `cargo fmt --check` and `git diff --check` passed; gate/CI own clippy, documented commands and broader suites after this explanation fix.

Realign after QA, 2026-11-14: the QA direction (longer Fable hold reported
behind an account-wide cooldown; `--clear-cooldown` naming remaining scoped
holds) is implemented. `lf account next` reaches
`provider_account::account_unavailable_reason` via `profile::show_next`
(`profile.rs:161`); `clear_account_cooldown` appends
`account_capacity_holds`. Same focused test passed again (1). Remaining: gate.

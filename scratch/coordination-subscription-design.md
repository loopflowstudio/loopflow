# Share identical subscription reads

2026-11-09 · optimize-plan · Iterate

## Decision and hypothesis

The runtime-only candidate is rejected: sequential runtime creation/destruction
is about 1 ms, while sixteen complete Session subscription cycles use a median
2,971 ms. Borrowing the server runtime also deadlocks when outer blocking pulls
saturate the same pool used by nested SQLite work.

Create one complete subscription reader per identical demand within one server
instance. Share its connection/schema lifetime, observer work, projections and
latest result. Keep delivery and checkpoints owned by each consumer. Target at
least 80% less server CPU for sixteen identical Session subscribers in both
active windows and complete admission/checkpoint/cancellation cycles, including
both cleanup tail windows. This is a testable target, not a promised result.

All rows of measure.html, subscription-measure.html and
subscription-followup.html were reviewed. The default n=5 Session baseline is
103.07 ms versus 6,551.27 ms active CPU per ten seconds for 1 versus 16 readers,
with 10 versus 160 projections and zero changed frames. Complete-cycle CPU is
33.21 versus 2,971.22 ms. Quiet CPU is 7.42 versus 54.14 ms. Profiles show schema
parsing and teardown contention; close callbacks precede actual schema freeing.

Bounded n=1 pilots establish broader paths, not stable timing estimates:
ProcessList admission/checkpoint is 12.48/3,562.27 ms for 1/16 readers; identical
TaskGet is 65.81/8,781.86 ms. Sixteen identical Task readers also spend 4,057.53 ms
quiet server CPU plus 2,135 ms child CPU. Distinct Task demand remains expensive
and is a control, not a claimed beneficiary of identical-demand sharing.

## Shape

Extend the existing server-owned connection/service state with a demand registry;
do not add a daemon, global cache, parallel subscription implementation, or
provider ownership layer. The registry belongs to the server lifetime and is
shared across its physical client connections. Keys contain the complete Read,
select, and any environment/database/context needed to distinguish results.
Conservative exact key equality is sufficient; do not canonicalize away meaning.
Including select in the key preserves selected-frame size behavior.

Each live key owns exactly one existing production Subscription and at most one
in-flight pull. Its latest result has bounded retention; consumers do not create
their own Reader, store, runtime or observer wake thread. Use the existing
Reader's pinned snapshots, configuration tracking, revision observation,
liveness probes, coalescing/rest and retry behavior. Delete the replaced
per-client execution path. Standalone CLI/embedding subscriptions keep their
existing semantics; this experiment changes server demand ownership only.

Keep independent SQLite execution capacity: do not nest store work into the same
saturated server blocking pool. Keep the existing reader runtime shape for this
experiment, so runtime tuning is not mixed into the sharing hypothesis.

## Freshness and lifecycle contract

- Every consumer gets an initial valid reading and independent acknowledgement.
  A join/checkpoint requires a read begun after its request; it cannot be
  satisfied by an older in-flight result. Coalesce concurrent demand into one
  in-flight read plus at most one trailing refresh, with explicit internal
  generation/ordering that is separate from clients' checkpoint numbers.
- Client checkpoint IDs need not be monotonic or unique across clients. Preserve
  each consumer's newest requested checkpoint in its frame. A sibling's
  checkpoint must not appear in another consumer's frame or force an unchanged
  unsolicited reading to that consumer.
- The producer must continue receiving/coalescing demand while a blocking read
  runs. Pending demand is bounded; do not queue one expensive read per request.
- Cancellation of one consumer preserves every sibling. Last demand removes the
  entry and cancels queued setup/pulls before expensive construction begins.
  Admitted blocking work may finish; discard its result and release everything.
  Rejoining a key while an old admitted job drains cannot acquire the old result.
- Abort/drop queued spawn_blocking work explicitly. The known ignored
  queued_subscription_cancellation_skips_construction test must pass and be
  enabled. Cover cancellation during admitted setup and pull as well.
- Slow clients retain bounded latest state and do not block producer/healthy
  peers. Preserve request-ID reuse isolation already enforced by task identity.
  Errors/recovery, missing/refused stores, selection and final reading before
  ended retain their existing semantics. Ended/error entries cannot poison a
  later subscription.
- Last demand releases readers, stores, wake threads, pending work and registry
  entries. Do not introduce a grace cache that keeps polling without consumers.
  One service's lifetime must not retain another service/test's demand.
- Viewer cancellation never ends provider/terminal owners, changes attachment
  authority, or discards an admitted mutation.

Choose the smallest implementation that establishes these invariants; the
specific channel/guard representation is an implementation decision.

## Regression coverage

Exercise actual server admission, Subscription creation and pulls, not merely
hypothetical executor objects. Use deterministic barriers and work/resource
counts rather than narrow CPU/wall-time assertions.

Prove one shared computation for concurrent identical demand through one and
multiple connections; different arguments/select/context do not alias. Prove
one trailing read when checkpoints arrive during an in-flight read, independent
checkpoint IDs/delivery, cancellation during queued/admitted setup/read, sibling
progress under a saturated outer blocking pool, immediate request-ID/key reuse,
slow consumer isolation, terminal/error recovery, and last-consumer cleanup
across repeated churn. Inspect production paths in the tests.

Replace the hypothetical executor saturation tests with production-path
regressions where possible; keep design probes opt-in if still useful.
Strengthen teardown acceptance with completed work/drop evidence, not just
SQLITE_TRACE_CLOSE. If measurement instrumentation changes, freeze/rerun a
matching baseline rather than merging unlike measurements.

Separately reproduce and repair the orphaned TaskUpdates candidate finding in
coordination-review-findings.md. Assert recovered one-query settled budget and
continued real update delivery after observed membership/Graph removal. Preserve
candidates across failed partial observations; clear on definitive ineligibility.
Commit this small correctness repair separately from the server sharing change.

## Measurement and keep/reject decision

Keep frozen 17c304389 server and all existing receipts. First use focused behavior
tests and a bounded smoke run. Optimize step 4 then compares alternating A/B
binaries with the frozen focus-v1.py workload, five paired samples per 1/16-reader
shape, reversing order. Include server CPU through both tail windows, child CPU,
first-reading/checkpoint/change latency, full outputs, writer receipts, SQL,
projection/store/runtime counts and final resource release. Count overhead from
the complete server path, not only the shared Reader.

The existing driver imports benchmark helpers: freeze/check all such inputs and
make sure fixture/service wiring changes cannot silently alter the baseline.
If required harness changes affect the measurement path, create a new matched
baseline/candidate freeze. Do not tune the workload to make sharing look better.

Repeat targeted identical/distinct Task and Process pilots for candidate control;
use paired repetitions for any claimed timing gain. Verify capped many-socket
and churn shapes with work budgets, reusing the existing 25 baseline samples for
their original scopes. Do not rerun every long matrix without a concrete need.

Keep when sixteen identical Session active and complete-cycle median server CPU
fall at least 80%, the repeated projection count is independent of subscriber
count (about 10 for ten spaced writes), and all functional/lifecycle checks pass.
Investigate single-reader or distinct-demand CPU/latency increases exceeding
10% beyond observed variation; do not hide child CPU or trade CPU for stale
results. Report ranges and paired differences, not a cross-configuration ratio.
This target is an experiment criterion, not a flaky CI timing threshold.

Distinct Task quiet/child work remains the next likely limiter. After this
candidate, return to optimize-plan with the remaining measured cost; narrower
invalidation/common checkout facts may remove that work. Do not claim near-zero
total orchestration or installed-machine recovery from this fixture result.

Preserve scratch research and all measurement/review artifacts. No installed
binary replacement, provider restart, priority change, new Task or worktree.

## Implementation cut — 2026-11-09

Delete — do not maintain: `server::service::subscribe`'s per-client construction,
blocking pull loop and checkpoint queue; replace them with the server's shared
demand registry and independent consumer delivery. Keep `Subscription`, Reader's
separate runtime, snapshot/config/retry logic and CLI/embedding entry points.
Remove the two hypothetical executor tests in `subscription_measurement.rs` once
production-path saturation coverage replaces them. Keep frozen copies unchanged.

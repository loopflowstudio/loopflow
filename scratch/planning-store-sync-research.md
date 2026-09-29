# Planning store research: Apollo, Relay, Realm and PowerSync

2026-09-29. Jack Heart requested this research during LOO-334's interactive
[review](repository-planning-connection-review.md). The
[working design](resolve-tasks-from-linear-and.md) records accepted decisions.
Research conclusions below distinguish observations from their application.
Jack subsequently approved proceeding; the working design adopts the shared-store
architecture. That approval does not select a dependency or settle outage policy.

## Finding

Apollo iOS provides the closest precedent: a common local object store, stable
identities across queries, explicit fetch policy and observable changes. Realm
and PowerSync show what full offline write replication adds. Loopflow can adopt
the shared-object pattern in its existing Rust/SQLite owner without selecting a
new library, GraphQL execution engine or synchronization service.

## Primary-source observations

**Apollo iOS:** GraphQL results normalize into records. Configured entity keys
let different queries update the same object; without them, response-path identity
can keep the same entity at different paths. Persistent SQLite and memory caches
implement the same cache interface. Persistence alone does not prove freshness.
[Normalization](https://www.apollographql.com/docs/ios/caching/introduction),
[storage](https://www.apollographql.com/docs/ios/caching/cache-setup).

Apollo's `cacheFirst` fetches on a miss; `networkFirst` falls back after network
failure; `cacheAndNetwork` can return local data followed by server data.
`networkOnly` still writes results into the cache by default. Watchers receive
relevant cache changes. [Query policies](https://www.apollographql.com/docs/ios/fetching/queries).

Apollo Client's optimistic mutations use provisional data distinct from confirmed
records; server completion replaces it, while failure rolls it back. This finding
is from the web client, not a claim of identical iOS APIs. Cache eviction and
garbage collection remove local data without establishing server deletion.
[Optimism](https://www.apollographql.com/docs/react/performance/optimistic-ui),
[eviction](https://www.apollographql.com/docs/react/caching/garbage-collection).

**Relay:** availability and freshness are separate. `store-or-network` fetches
missing/stale data; `store-and-network` reuses local data while fetching. Records
can be absent because a query was never fetched or because they were collected.
Invalidating a present record marks it stale and causes a subsequent request to
refetch; it does not immediately force every mounted reader to fetch. Cache
invalidation in this vocabulary does not mean invalid Task execution.
[Policies](https://relay.dev/docs/guided-tour/reusing-cached-data/fetch-policies/),
[presence](https://relay.dev/docs/guided-tour/reusing-cached-data/presence-of-data/),
[staleness](https://relay.dev/docs/guided-tour/reusing-cached-data/staleness-of-data/).

**Realm / historical Atlas Device Sync:** concurrent offline writers converged
under explicit rules: object deletion wins over edits, last update wins for a
property, and primary keys identify objects. These are replication rules, not
judgments about whether Task work remains meaningful.
[Conflict semantics](https://www.mongodb.com/docs/atlas/app-services/sync/details/conflict-resolution/).
Atlas Device Sync reached end of life on September 30, 2025; study it as a
precedent, not an available hosted dependency. This does not establish that all
local Realm database code disappeared.
[MongoDB EOL notice](https://www.mongodb.com/docs/api/doc/atlas-app-services-admin-api-v3).

**PowerSync:** local SQLite reads and writes sit alongside an upload queue and
authoritative backend validation. Its documented checkpoint protocol applies
pending local writes over downloaded server state and advances once uploads are
acknowledged and the corresponding server checkpoint is downloaded. The backend
can reject or resolve changes. Server authority and offline local writes can
coexist, but require machinery beyond a query cache.
[Architecture](https://powersync.com/blog/introducing-powersync-v1-0-postgres-sqlite-sync-layer),
[SQLite overview](https://powersync.com/sync-postgres).

PowerSync's architecture relies on database replication and its sync protocol.
Linear exposes GraphQL and webhooks, including create/update/remove events and
delivery IDs. Those APIs do not by themselves establish equivalent transaction
checkpoints or a complete ordered replication log.
[Linear webhooks](https://linear.app/developers/webhooks).

## Repository evidence

- `store/mod.rs::PmSnapshotRow` and `store/sqlite.rs::put_pm_snapshot` persist a
  serialized payload per Wave, atomically replaced on refresh.
- `ops/pm.rs::plan_snapshot_read` already separates cached reads from refresh
  and hard/soft refresh failure. `try_timed_refresh` bounds network work.
- `ops/task_pm.rs::resolve_task_async` searches cached Wave membership before
  refreshing. `ops/task.rs::task_status` separately requires an execution Task.
- Repository configuration already owns provider/Team; Wave Initiative IDs
  identify relationships inside that connection.

The current cache already has useful refresh policy. The missing architectural
piece is a shared entity reader that does not require prior list membership or
allocated execution. Normalization must replace competing snapshot readers,
not become an additional planning copy.

## Proposed application

1. Normalize Tasks by stable entity identity in existing SQLite. Detail lookup,
   Wave listing, mutation response and webhook update the same planning record.
   Membership/order reference it rather than duplicate title/status.
2. Keep acquisition before the shared reader. Connected lookup refreshes into
   local planning, then follows the same read path as an unconnected repository.
   Reuse bounded PM refresh; callers should not each choose whether to call Linear.
3. Separate missing local data, stale observations and invalid Task planning.
   A partial list cannot prove removal. Omitted fields cannot clear known values.
   Prefer complete domain-entity refreshes over a generic GraphQL field cache.
4. Confirm provider writes before they govern managed execution. Reuse existing
   idempotent mutation handling; no optimistic creation may launch a worker before
   acceptance. A durable offline queue remains outside the proposed scope.
5. Let committed local changes update readers through existing app refresh or
   subscription paths. Keep Rust's store as owner, avoiding an independent Swift
   planning cache with its own identity and reconciliation rules.
6. Retain Jack's invalidation rule: known missing/ineligible Task planning stops
   managed progression. Ordinary worktree Flows remain independent. Database sync
   algorithms do not supply execution reconciliation or require it.

Add proofs that detail/list reads converge on one identity, mutations update both
views, partial lists do not erase Tasks, older observations do not resurrect
confirmed removal, and failed refresh preserves last-good data and its age.

## Open choice and limits

The sources make offline policy explicit; none chooses it for Loopflow. Jack's
direction supports local reads with sync when available, but does not yet decide
whether cached Tasks may start/resume during outages. Keep that policy at the
sync/managed-admission boundary; do not add user-facing flags merely because these
libraries offer them.

This is documentation and source research, not a library integration, benchmark
or configured provider test. No dependency was selected. Linear transactional
snapshot semantics and transport ordering were not established.

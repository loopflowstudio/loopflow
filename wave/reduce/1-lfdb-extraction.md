# lfdb — persistence as shared infra

**Finish line:** `crate::lfd::store` (backends + migrations + persisted domain
types + the registry API) is lifted out of `lfd` into a bounded `crate::lfdb`
module that `lf d`, `lf q`, and the shrunk `lfd serve` all consume as a peer.
Persistence is no longer lfd-owned. Nothing outside `lfdb` reaches into a SQLite
or Postgres row directly.

## Why this is first

`lf d` and `lf q` read/write the same tables (waves, runs, sessions,
terminal_sessions, credentials, queue state). They need one backend, not two
reach-ins. Extracting the store is the foundational move the rest of the wave
stands on — and it frontloads the risk: if the seam between "persisted types"
and "wire DTOs" is wrong, everything downstream inherits it. Rename first,
extract to a workspace crate when the seams are proven.

## Scope

- Lift `rust/loopflow/src/lfd/store/` (`sqlite.rs`, `postgres.rs`, `rows.rs`,
  `migrations/`, `catalog.rs`, `token_crypto.rs`) into `crate::lfdb`. lfd becomes
  one more `lfdb` client — the one that watches it and pushes subscriptions.
- **Boundary — persisted types vs wire DTOs.** `lfdb` owns
  `Wave`/`Run`/`Session`/`RepoWork` (the *storage* shape). The HTTP DTOs stay
  with the shrunk `lfd serve` (the *subscription* shape), keeping the DTO-drift
  rules scoped to the one surface that still crosses a network.
- The migration-idempotency fix already in `store/migrations.rs` (the renamed
  additive migration that converged on this branch) is `lfdb` code in waiting —
  it moves with the store.

## Done when

- `lfdb` is a bounded module with no `lfd`-specific imports; `lfd` imports `lfdb`,
  not the reverse.
- Store types (`Wave`/`Run`/`Session`/`RepoWork`) live in `lfdb`; wire DTOs stay
  in the server crate.
- All existing store tests pass unmoved (or moved wholesale into `lfdb`).
- Net line count does not grow — this is a lift, not a rewrite.

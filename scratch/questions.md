# LOO-868 questions

- `export_peer_planning` (LOO-859, listed under Exclusions) changed from
  `conn.transaction()` to an explicit `Deferred` transaction. The store's default
  behavior is `IMMEDIATE`, so its read phase held the write lock across the full
  `export_rows` scan, contrary to its own comment. One-word fix on the same
  acquire path; revert it to a separate Task if the exclusion should hold.

# Store layer deduplication

The store has two backends (SQLite and Postgres) that share identical business logic with different async syntax. Today this means copy-pasting every non-trivial store operation across `sqlite.rs` and `postgres.rs`. As the store grows, so does the duplication — and the drift risk.

Example: `join_waves` has five match arms, ~70 lines, duplicated verbatim. `leave_wave`, `create_wave` (with child walking), and the recursive CTE tree loading all follow the same pattern.

## Approach

Introduce a thin internal trait for the primitive store operations (`upsert_wave`, `delete_wave`, `delete_stimuli_for_wave`, `load_wave_tree`). Composite operations (`join_waves`, `leave_wave`, future chord/beat-grid ops) are written once against this trait. Both backends implement the primitives; the shared logic lives in `store/mod.rs` or a new `store/ops.rs`.

## Audit

Walk every method on `WaveStateStore` and `ExecutionStore`. For each:
- If SQLite and Postgres implementations are identical modulo async → extract to shared function
- If they differ only in SQL dialect → keep separate but document the divergence
- If they share logic with small dialect differences → extract the logic, parameterize the SQL

## Success

Adding a new store operation means writing the business logic once and the SQL twice (if it differs). No more copy-pasting match arms or multi-step flows.

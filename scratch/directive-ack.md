# Directive v1 acknowledgment (blocked on store; replay when possible)

`lf task acknowledge W2-218 --directive 1` could not run: the installed binary
cannot open the incompatible live store (the incident under repair). Replay this
summary once a store-capable binary exists:

> Sharpened, not redirected. Recovery generalizes the existing single-lineage
> converger (DIVERGENT_MIGRATIONS/CONVERGED_VERSIONS) into a permutation-general
> converger keyed on migration name + content-checksum instead of hardcoding a
> second lineage — the live ledger is the same 5 migrations with swapped
> ordinals, so name-set + full-schema equality recognizes it and every future
> permutation without new per-incident code. Prerequisite surfaced:
> product_schema() today compares only table + column names, so it must become a
> complete fingerprint (types, NOT NULL, PK, indexes, triggers, FKs) before any
> generalized convergence can be trusted. Split into two serial PRs under this
> Task: PR1 = recovery + tolerance + history-fingerprinted backup + sanitized
> fixture (lands first to unblock the live DB); PR2 = authoring-boundary
> prevention = checksum + provenance columns on schema_migrations,
> origin/main-based ordinal allocation in new_migration.py, and a
> convergence-matrix gate in check_migrations.py. Excluded as follow-up Tasks
> (the "store decomposition" the directive carves out): telemetry-optional trace
> capture, store-free `lf pr land`, store capability boundaries, separate
> databases, `lf store clone`.

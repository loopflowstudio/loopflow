# Assumptions

- 2026-10-09: The brief's no-migration/shipped-record boundary takes precedence
  over its unqualified zero-match search. Existing SQL (including draft SQL) stays
  byte-identical; acceptance searches exclude migrations. Historical profiling
  captures also retain their recorded symbols. Product help/errors and builtin
  skills use Session rather than exposing the renamed Rust type.

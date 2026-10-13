# Open local-build questions

- Writer exclusion remains unresolved: Cargo clean replaces a held lock; Swift
  clean/reset behavior is unproved. Dependent implementation waits for the
  [design's exclusion revision](local-build-speed-space.md#writer-exclusion-unresolved).
  Always retaining Rust output would leave bounded lifetime unmet. Ordinary direct
  Cargo clean/build preservation remains required unless Jack Heart accepts a
  narrower participating-launcher contract. Swift clean/reset and node_modules
  writer exclusion also remain unproved. Exact-CLI preparation is independent.
- Broader Infrastructure overlap remains unknown after failed plan inspection;
  the design records the confirmed sibling scope.

The [research](local-build-research.md) owns cold-cache assumptions and probe
evidence. The [design](local-build-speed-space.md) owns the October 13 draft policy:
24h without modification, eligibility at the next supported invocation, proven
writer exclusion per output type, and history/store/install exclusions.

The installed `lf context` omits relocated `.lf/wave/infrastructure/MEMORY.md`.
Memory is measured separately with cl100k_base against its reported 16,000-token /
131,072-byte defaults; scratch uses the normal query. No limits were changed.

- `import_peer_planning` skips a revision already recorded in
  `planning_peer_imports` when the repository has no active conflicts. Assumed:
  re-importing the same remote revision changes nothing then (local edits are
  newer by clock; conflicts force a retry). Revisit if an import ever needs to
  re-run for local-only reasons.
- Once the server writes `sqlite_stat1`, every statement may plan differently.
  `store::query_plans` covers the boundary, retirement, viewer and planning
  write paths both ways; other paths are unguarded under measured stats.
- Incremental peer import treats a change as seen when it is routed from the
  destination, and re-merges every incoming change of any field that has an
  unseen one (parents share their field). Active conflicts force the full
  import, which recomputes them.

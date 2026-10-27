# LOO-610 open questions and assumptions

- **`repo identity --bind/--relocate` kept** (2026-10-26). The plan said delete
  them in favour of Git-locator resolution, but `lf machine` rentals run
  `lf repository --bind <id>` on the clone, and `--repository ID` routing fails
  without that association. Deleting them needs repository identity resolved
  by Git locator first. They moved unchanged to `lf config repository`; the
  deletion belongs with the planning-surface follow-up.
- **`repo planning` → `lf config planning {key,status}`**, not
  `self doctor --planning`. Doctor's JSON carries no planning destinations, so
  Desktop's peer-planning view had nowhere to read from. Jack placed planning
  key/status under "probably work configuration".
- **`repo refresh` deleted; `wave status --sync` absorbed its Git branch.**
  `--sync` now exchanges the repository's Git planning ref when the repository
  plans through Git, else refreshes the Wave from Linear. `repo refresh --all`
  (every Wave from Linear in one call) has no replacement.
- **`lf sync` is a reserved shortcut to `ops sync`**, because `task sync` also
  exists and descendant elision would otherwise call it ambiguous.
- **`code` default keeps `workflow_name` Optional on the wire** (2026-10-26).
  Task reads now always report a Workflow (`code · builtin` at worst), so
  Desktop's "No workflow selected" fallback in `TaskWorkView` no longer shows.
  Making the DTO field required touches Rust, Swift and fixtures; left for the
  LOO-604 read redesign, which reshapes these projections anyway.

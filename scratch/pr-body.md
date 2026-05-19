## Try it!

```bash
cargo test --all
swift test --package-path swift
uv run pytest python/tests/
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
```

With Asana auth configured:

```bash
lf op pm list
uv run python scripts/verify_canonical_waves.py
```

Expected: `lf op pm list` shows projects from the configured `asana.team`; a local `wave/fake/fake.yaml` that is not an Asana project does not appear in discovered waves. A matching `~/.lf/config.yaml` `repos:` override changes discovery to that override team. The verification script creates a temporary Asana team, uses it as an override, restores local config, and deletes the temporary team.

## Intent

Make the configured Asana team the shared source of truth for which waves exist. This removes branch-local filesystem accidents from wave discovery and makes CLI, lfd, and Concerto agree across worktrees.

## Assumptions

- PM-backed wave discovery uses Asana for this milestone.
- `asana.team` may be absent temporarily so first discovery can find or create a `Loopflow` team in an organization workspace.
- `asana.workspace` is optional when a canonical team is configured; tooling can derive the organization workspace from that team when it needs to create a temporary verification team.
- The stale project cache is only a fallback after at least one successful Asana project fetch.

## Key decisions

- Rename config to `asana.team` and keep only a serde alias for `default_team` while existing configs migrate.
- Merge global `repos:<absolute repo path>` overrides after repo config so local dev teams do not require committed config edits.
- Persist autocreated team gids through the config resolution layer, not inside the Asana client.
- Delete Swift filesystem auto-bootstrap and re-register repos before discovered-wave refresh so lfd has the repo when listing discovered waves.

## Not included

- Origin-main config loading.
- Free-tier Asana workspace fallback.
- Concerto override editing UI.
- Strict PM mirror sync or local-draft UI treatment for non-Asana wave directories.

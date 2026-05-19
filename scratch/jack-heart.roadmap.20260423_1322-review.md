# Canonical Asana waves review

## What was implemented

Configured Asana teams are now the canonical wave set. `lf op pm list`, lfd discovered-wave routes, and Concerto sidebar refresh read projects from the resolved `asana.team` instead of trusting local `wave/<name>/` directories. Global `repos:` overrides merge over repo config, and missing `asana.team` can be autocreated and persisted through config resolution.

The branch also narrows this milestone to Asana-backed PM discovery, removes Linear/Notion PM provider code, and deletes Concerto's filesystem auto-bootstrap so stale local wave folders no longer invent waves.

## Key choices

- Use `asana.team` as the config name, with only a serde alias for `default_team` during config migration.
- Keep `wave/<name>/` as the local editing mirror, not the source of existence.
- Merge `~/.lf/config.yaml repos:<absolute repo path>` after repo config so local team overrides do not require committed edits.
- Persist autocreated team gids in config resolution rather than hiding that state inside the Asana client.
- Make `scripts/verify_canonical_waves.py` derive the workspace from the canonical team when `asana.workspace` is not configured, matching the docs that workspace pinning is optional.

## How it fits together

Config resolution produces an effective Asana workspace/team for a repo. PM discovery lists projects in that team and caches successful results only as a fallback. The CLI, lfd discovered-wave endpoints, and Concerto's repo state all use that same discovery path, so worktrees and UI agree even when local wave mirrors are missing or polluted.

## Risks and bottlenecks

- Live discovery depends on Asana availability and credentials; the stale project cache only helps after one successful fetch.
- Autocreating a team still requires an organization workspace and permissions to create teams.
- Local-only wave directories intentionally become unmanaged rather than first-class discovered waves.
- The live verification script creates and deletes a temporary Asana team; failures restore local config and best-effort cleanup the team.

## What's not included

- Origin-main config loading.
- Free-tier Asana workspace fallback.
- Concerto UI for editing repo override teams.
- Strict PM mirror sync or explicit local-draft treatment for non-Asana wave directories.

## Validation

- `cargo fmt --check` — pass
- `cargo clippy -- -D warnings` — pass
- `cargo test --all` — pass
- `uv run pytest python/tests/` — pass
- `swift test --package-path swift` — pass
- `tests/e2e/test_smoke.sh` — pass
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` — pass
- `uv run python -m py_compile scripts/verify_canonical_waves.py` — pass
- `cargo run --bin lf -- op pm list` — pass; listed Desktop, Mobile, Root, Workflows from team `1214267955480461`
- `LF_BIN=target/debug/lf uv run python scripts/verify_canonical_waves.py` — pass; verified repo override + canonical-team discovery against live Asana
- `rg "\.default_team\b" rust/ swift/ python/` — no hits
- `rg "default_team" rust/ swift/ python/ README.md docs/` — only the documented serde alias remains
- `rg "bootstrapRoadmapWavesIfNeeded|roadmapWaveNames\(" swift/` — no hits

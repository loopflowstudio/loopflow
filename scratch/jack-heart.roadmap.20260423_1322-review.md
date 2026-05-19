# Canonical Asana waves review

## What was implemented

Configured Asana teams now define the wave set for PM-backed repos. `asana.team` replaces `asana.default_team` as the canonical team fence, with a serde alias for existing configs. `lf op pm list`, `GET /v0/waves/discovered`, and Concerto discovery now read Asana team projects instead of proving wave existence from `wave/<name>/<name>.yaml`.

The branch also adds per-repo overrides under `~/.lf/config.yaml`, automatic Loopflow team resolution for organization workspaces, a stale `.lf/cache/workspace/projects.json` fallback, and `scripts/verify_canonical_waves.py` as the live validation walkthrough.

## Key choices

- Asana project membership is the only canonical wave-existence source. Local `wave/` directories remain editable mirrors and no longer bootstrap waves by accident.
- Repo-specific `repos:` overrides live in global config and merge after repo config so a local checkout can point at a dev team without changing committed config.
- Autocreated team gids are persisted by the config resolution caller, not by `AsanaClient`, keeping the provider client side-effect free.
- The discovered-waves DTO stayed mostly stable; only `stale` was added so UIs can distinguish cached Asana results from fresh results.
- During gate, the new config persistence tests were made deterministic by serializing `LF_HOME` mutation inside the config test module.

## How it fits together

`load_config_resolution(repo)` merges global config, repo config, and matching global `repos:<repo>` override, then records where an autocreated team should be written. `discover_waves(repo)` resolves that effective config, asks Asana for projects in the resolved team, writes the workspace cache, and maps each project into `DiscoveredWave`. The lfd route and Concerto consume that same discovered list, so CLI and UI agree on the wave set.

## Risks and bottlenecks

- Live discovery depends on Asana and local OAuth credentials; the stale cache protects outages after the first successful fetch but cannot help first-run resolution with no configured team.
- Per-repo override matching is exact on the canonicalized repo path; symlinked or differently mounted paths need matching override keys.
- The verification script mutates Asana by creating/deleting a temporary team and requires appropriate Asana permissions.

## What's not included

- Reading repo config from `origin/main` instead of the current worktree.
- Free-tier workspace-as-fence behavior for Asana workspaces without teams.
- Concerto UI for editing per-repo Asana overrides.
- Strict PM mirror synchronization or local-draft decoration for non-Asana `wave/` directories.

## Validation

- `cargo fmt --check` — pass
- `cargo clippy -- -D warnings` — pass
- `cargo test --all` — pass
- `uv run pytest python/tests/` — pass
- `swift test --package-path swift` — pass
- `tests/e2e/test_smoke.sh` — pass
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` — pass
- `uv run python -m py_compile scripts/verify_canonical_waves.py` — pass
- `rg "\.default_team\b" rust/ swift/ python/` — no hits
- `rg "default_team" rust/ swift/ python/ README.md docs/` — only the documented serde alias remains
- `rg "bootstrapRoadmapWavesIfNeeded|roadmapWaveNames\(" swift/` — no hits

`uv run python scripts/verify_canonical_waves.py` was not run because it requires live Asana credentials, sufficient team-management permissions, and a running local setup.

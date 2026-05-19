# Canonical Asana waves review — validation

Implementation summary and forward-looking follow-ups are folded into
`wave/workflows/3-wave-discovery-and-root-chord.md` and
`wave/workflows/2-pm-round-trip.md`. This file retains only the validation
record for reviewers landing on this branch.

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

`scripts/verify_canonical_waves.py` was not run in-branch: it requires live
Asana credentials, team-management permissions, and a running local setup. Run
it before relying on the override/canonical paths in a live environment.

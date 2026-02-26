# 04: Repo Onboarding

Status: **shipped** (branch `jack-heart.auth.20260226_1309`)

## What shipped

- Migration `016_repos` creates `repos` table with path as primary key.
- `RepoStore` trait (list, get, upsert, delete) with sqlite and postgres implementations, following the `ChordStore` pattern.
- `Repo` type with `path`, `name`, `added_at` fields.
- `POST /v0/repos` — validates absolute path, checks `.git` exists, canonicalizes, upserts.
- `DELETE /v0/repos` — body-based delete (paths don't encode cleanly in URLs).
- `GET /v0/repos` — merges registered repos with wave-derived repos, returns `registered` flag and wave counts.
- Python client: `list_repos`, `add_repo`, `remove_repo` on `Client`.
- Python API: `list_repos`, `add_repo`, `remove_repo` module wrappers.
- Python CLI: `lfq repos`, `lfq repos add`, `lfq repos rm`.
- Python models: `Repo` with `path`, `name`, `wave_count`, `registered`, `added_at`.
- Swift: `listRepos`, `addRepo`, `removeRepo` on `WaveServiceProtocol` / `LocalWaveService`.
- 24 Rust tests, 70+ Python tests passing.

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test -p loopflow repo` — 24 tests
- `uv run pytest python/tests/` — 70 tests

## Follow-ups

- **Concerto PortfolioService integration**: `PortfolioService` should call `POST /v0/repos` on add and read from `GET /v0/repos` as source of truth. Currently out of scope.
- **API contract alignment**: Implementation uses path-based `POST/DELETE /v0/repos`. Area docs may describe a name-based `register/add/unregister` design. Confirm which contract is the long-term target before follow-up iterations.
- **Wave count scaling**: `list_repos_handler` fetches all waves to count per-repo. A `COUNT GROUP BY repo` query would be more efficient at scale.
- **Canonicalization edge case on remove**: If a repo directory is deleted and the user sends a non-canonical path, the delete won't match. Narrow edge case since `GET` returns canonical paths.

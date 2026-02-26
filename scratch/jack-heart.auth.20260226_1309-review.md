# Repo Onboarding — Design Review

## What was implemented

Server-side repo registration. `POST /v0/repos` registers a git repo by path, `DELETE /v0/repos` removes it, `GET /v0/repos` returns the union of registered repos and wave-derived repos. Full stack: migration, store trait, sqlite/postgres implementations, HTTP handlers, Python client + API + CLI, Swift client.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|----------------------|
| Path as primary key | A repo's identity is its location — no synthetic ID needed | UUID would add indirection with zero benefit |
| Merge registered + wave-derived in GET | Backwards compatible — existing users see the same repos | Separate endpoints would split the source of truth |
| DELETE with request body | Paths contain `/` — URL encoding is fragile | URL-encoded path param would be ambiguous |
| Server-side git validation on POST | Catches mistakes early, validates the path exists on the server | Client-side validation wouldn't help for remote deployments |
| Canonicalize on add, best-effort on remove | Ensures consistent storage; allows removing repos whose dirs are gone | Strict canonicalize on both would block removing deleted repos |

## How it fits together

```
POST /v0/repos {path} → validate → canonicalize → check .git → upsert repos table → RepoDto
GET  /v0/repos         → repos table ∪ wave-derived repos → sorted Vec<RepoDto>
DELETE /v0/repos {path} → validate → normalize → delete from repos table → 204
```

The `RepoStore` trait follows the `ChordStore` pattern: 4 methods (`list`, `get`, `upsert`, `delete`), implemented identically for sqlite and postgres, delegated via `Store::repos()`. Migration `016_repos` creates the table.

Python: `Client.list_repos/add_repo/remove_repo` → `api` module wrappers → `lfq repos [add|rm]` CLI.

Swift: `WaveServiceProtocol` gains `listRepos/addRepo/removeRepo`, implemented in `LocalWaveService` with the same HTTP calls.

## Risks and bottlenecks

- **Wave list for counting**: `list_repos_handler` fetches all waves to count per-repo. At scale (thousands of waves), this could be slow. A `COUNT GROUP BY repo` query would be more efficient but requires a new store method. Fine for now.
- **Canonicalization mismatch on remove**: If a repo was registered with a canonical path and the user sends a non-canonical path (e.g., with symlinks) after the directory is deleted, the delete won't find the row. Users listing repos first (via GET) get the canonical path back, so this is a narrow edge case.

## What's not included

- Concerto `PortfolioService` integration (out of scope per design doc — stays as a follow-up)
- Repo-level settings or configuration
- Auth provider linking to repos
- `RepoScanner` changes (stays client-side)

## Gate results

- `cargo fmt --check` — pass (one formatting fix applied)
- `cargo clippy -- -D warnings` — pass
- `cargo test -p loopflow repo` — 24 tests passed
- `uv run pytest python/tests/` — 70 tests passed

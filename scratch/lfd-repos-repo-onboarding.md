# Repo Onboarding

## Problem

Repos are second-class citizens. They exist implicitly as properties of waves — `GET /v0/repos` aggregates `wave.repo` paths, and Concerto persists them client-side in UserDefaults via `PortfolioService`. This means:

- A repo with zero waves is invisible to the server
- Adding a repo in Concerto doesn't tell lfd about it
- Repo state doesn't survive Concerto reinstalls or sync to new devices
- There's no validation that a path is actually a git repo until a wave run fails

The repo-first workflow (add a repo, then create waves in it) needs server-side repo registration.

## Approach

Add a `repos` table to the store. `POST /v0/repos` registers a repo, `DELETE /v0/repos` removes it. `GET /v0/repos` returns the union of explicitly registered repos and wave-derived repos, so nothing breaks for existing users.

Concerto's `PortfolioService` becomes a thin client that calls `POST /v0/repos` on add and reads from `GET /v0/repos` as source of truth. `RepoScanner` stays client-side — it discovers repos locally, then the user explicitly registers them via the API.

### New `RepoStore` trait

Follows the `ChordStore` pattern — separate trait, separate concern:

```rust
#[async_trait::async_trait]
pub trait RepoStore: Send + Sync {
    async fn list_repos(&self) -> StoreResult<Vec<Repo>>;
    async fn get_repo(&self, path: &str) -> StoreResult<Option<Repo>>;
    async fn upsert_repo(&self, repo: &Repo) -> StoreResult<()>;
    async fn delete_repo(&self, path: &str) -> StoreResult<()>;
}
```

### New `Repo` type

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub path: String,           // canonicalized absolute path (primary key)
    pub name: String,           // last path component
    pub added_at: OffsetDateTime,
}
```

Path is the primary key — no synthetic ID. Repos are identified by where they live on disk.

### Migration `012_repos`

```sql
CREATE TABLE IF NOT EXISTS repos (
    path TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    added_at TEXT NOT NULL
);
```

### HTTP endpoints

| Method | Path | Body | Returns |
|--------|------|------|---------|
| `GET /v0/repos` | — | — | `ListResponse<RepoDto>` (merged: registered + wave-derived) |
| `POST /v0/repos` | `{ "path": "/abs/path" }` | — | `RepoDto` (201 Created) |
| `DELETE /v0/repos` | `{ "path": "/abs/path" }` | — | 204 No Content |

DELETE uses request body instead of URL-encoded path because repo paths contain `/` and URL encoding them is fragile. This matches how the Slack API handles similar cases.

### Merge strategy for GET

The existing handler aggregates repos from waves. The new handler merges both sources:

1. Fetch registered repos from `repos` table
2. Fetch all waves, group by repo path, count per repo
3. Union by path: registered repos get wave counts; wave-only repos appear with `registered: false`
4. Sort alphabetically by path

`RepoDto` gains one field:

```rust
pub struct RepoDto {
    pub object: String,        // "repo"
    pub path: String,
    pub name: String,
    pub wave_count: u32,
    pub registered: bool,      // explicitly registered via POST
    pub added_at: Option<String>, // ISO 8601, None for wave-only repos
}
```

### Validation

`POST /v0/repos` validates:
1. Path is absolute
2. Path exists on the server's filesystem
3. Path contains `.git/` directory or `.git` file (worktree pointer)
4. Path is canonicalized (resolve symlinks) before storage

Validation errors return 422 with a descriptive message.

### Concerto changes

`PortfolioService` splits into two roles:

- **Server sync**: `addRepo` calls `POST /v0/repos`, `loadRepos` calls `GET /v0/repos`
- **Offline cache**: UserDefaults stores the last-known repo list for instant launch before the server responds

`WaveService` gains two methods:
```swift
func addRepo(path: String) async throws -> RemoteRepo
func removeRepo(path: String) async throws
```

`PortfolioWindow` calls `addRepo` when the user selects a repo from the scanner, instead of only calling `portfolioService.addRepo`.

### Python client

```python
# client.py
def list_repos(self) -> list[Repo]:
    payload = self._request_json("GET", "/v0/repos")
    return self._parse_model_list(payload, Repo)

def add_repo(self, path: str) -> Repo:
    payload = self._request_json("POST", "/v0/repos", json={"path": path})
    return Repo.model_validate(payload)

def remove_repo(self, path: str) -> None:
    self._request_json("DELETE", "/v0/repos", json={"path": path})
```

```python
# models.py
class Repo(BaseModel):
    path: str
    name: str
    wave_count: int
    registered: bool
    added_at: Optional[datetime] = None
```

### `lfq repos` CLI

```
lfq repos                    # list repos (default)
lfq repos add /path/to/repo  # register a repo
lfq repos rm /path/to/repo   # unregister a repo
```

Table columns: name, path, waves, registered, added.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Config file persistence | Simpler, no migration needed | Doesn't work with Postgres backend; not transactional; diverges from every other entity's pattern |
| Phantom waves (zero-config "wave" per repo) | No new table or trait | Pollutes wave semantics; waves have required fields (flow) that don't apply to repos |
| URL-encoded path in DELETE route | More RESTful | Paths with `/` produce ambiguous URLs; body-based delete is unambiguous |
| Separate `id` column (UUID) | Matches wave/chord pattern | Repos are uniquely identified by path — a synthetic ID adds indirection with no benefit |

## Key decisions

1. **Path as primary key.** No synthetic ID. A repo's identity is its location. This eliminates a whole class of bugs where two entries point to the same directory.

2. **Merge, don't migrate.** `GET /v0/repos` returns both registered and wave-derived repos. Existing users see the same repos they always did. New repos appear as registered. No breaking change.

3. **DELETE with request body.** Unconventional but correct. Paths are the identifier and they don't encode cleanly in URLs.

4. **Server-side validation.** The server checks that the path is a git repo. For remote deployments, this validates the path exists on the remote machine, catching user mistakes early.

5. **PortfolioService stays but becomes a sync layer.** UserDefaults provides instant launch state while `GET /v0/repos` loads in the background. No jarring empty-then-populated flash.

## Scope

**In scope:**
- `repos` table, migration, store trait + sqlite/postgres implementations
- `POST /v0/repos`, `DELETE /v0/repos`, updated `GET /v0/repos`
- `WaveService.addRepo/removeRepo` in Swift
- `PortfolioService` calls server on add/remove
- Python `list_repos`, `add_repo`, `remove_repo`
- `lfq repos` / `lfq repos add` / `lfq repos rm`
- Tests for all layers

**Out of scope:**
- `RepoScanner` changes (stays macOS-only, client-side)
- Auth provider linking to repos
- Token refresh / expiry polling
- Repo-level settings or configuration
- Remote repo cloning or git operations

## Done when

```bash
# Server persists repos across restart
cargo test -p loopflow repo
lfd &
curl -X POST localhost:2486/v0/repos -H 'Content-Type: application/json' -d '{"path":"/tmp/test-repo"}'
# restart lfd
curl localhost:2486/v0/repos | jq '.data[] | select(.path=="/tmp/test-repo")'

# Python client
uv run pytest python/tests/test_client.py -k repo

# CLI
lfq repos add /tmp/test-repo
lfq repos
lfq repos rm /tmp/test-repo
```

Wave goal advanced: "Concerto portfolio uses lfd as source of truth (not just UserDefaults)" and "Repos persist across lfd restarts."

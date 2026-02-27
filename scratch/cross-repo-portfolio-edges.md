# Portfolio DAG Edges

## Problem

lfd's portfolio is a flat list of repos identified by filesystem path. Cross-repo features (context loading, stimulus, commits) need stable identity and directed relationships. Studio can't reference loopflow as a child repo — there's no concept of edges, and paths break across machines.

GitHub `owner/repo` is the natural stable identity. Every repo in the portfolio has a GitHub remote. Building on this, a DAG of parent→child edges lets lfd know which repos are related and in what direction.

## Approach

Add `RepoId` (`owner/repo`) to the `Repo` type, derived from the git remote at `add_repo` time. Add a `repo_edges` table for directed parent→child relationships. Expose child CRUD and traversal via the HTTP API. Add Python client methods and lfq CLI commands.

### RepoId

A newtype wrapping `owner/repo`, derived by calling the existing `github_repo_from_local()` in `lfd/github.rs`. Repos without a GitHub remote are rejected at `add_repo` time (this is a behavior change — the validation already ensures it's a git repo, we now also require a GitHub remote).

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoId(String);

impl RepoId {
    pub fn as_str(&self) -> &str { &self.0 }
}

impl fmt::Display for RepoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}
```

Added to `Repo`:

```rust
pub struct Repo {
    pub path: String,
    pub repo_id: RepoId,     // new — derived from GitHub remote
    pub name: String,
    pub added_at: OffsetDateTime,
}
```

`path` stays as the database primary key (it's machine-local, used for filesystem operations). `repo_id` is an additional `NOT NULL UNIQUE` column. Edges reference `repo_id`, not `path`.

### Schema

Migration `021_repo_edges.sql`. Drop and recreate the repos table with the new column. No backwards compatibility — existing repos are re-added by the user.

```sql
DROP TABLE IF EXISTS repos;
CREATE TABLE IF NOT EXISTS repos (
    path TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL,
    name TEXT NOT NULL,
    added_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_repos_repo_id ON repos(repo_id);

CREATE TABLE IF NOT EXISTS repo_edges (
    parent_repo_id TEXT NOT NULL,
    child_repo_id TEXT NOT NULL,
    PRIMARY KEY (parent_repo_id, child_repo_id)
);
```

### Store

Extend `RepoStore`:

```rust
#[async_trait::async_trait]
pub trait RepoStore: Send + Sync {
    // existing
    async fn list_repos(&self) -> StoreResult<Vec<Repo>>;
    async fn get_repo(&self, path: &str) -> StoreResult<Option<Repo>>;
    async fn upsert_repo(&self, repo: &Repo) -> StoreResult<()>;
    async fn delete_repo(&self, path: &str) -> StoreResult<()>;

    // new
    async fn get_repo_by_repo_id(&self, repo_id: &RepoId) -> StoreResult<Option<Repo>>;
    async fn list_edges(&self) -> StoreResult<Vec<RepoEdge>>;
    async fn add_edge(&self, edge: &RepoEdge) -> StoreResult<()>;
    async fn remove_edge(&self, parent_id: &RepoId, child_id: &RepoId) -> StoreResult<()>;
    async fn children(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>>;
    async fn parents(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>>;
}
```

`children` and `parents` JOIN `repo_edges` with `repos` to return full `Repo` objects.

### Cycle detection

On `add_child(parent, child)`: load all edges, DFS from `child` following forward edges. If `parent` is reachable, the new edge would create a cycle — reject with 422.

Concurrent `POST` calls could theoretically both pass the check and create a cycle (TOCTOU). For <100 repos this is zero practical risk. Comment in code, don't solve with a lock.

```rust
fn would_create_cycle(edges: &[RepoEdge], new_parent: &RepoId, new_child: &RepoId) -> bool {
    let mut visited = HashSet::new();
    let mut stack = vec![new_child.clone()];
    while let Some(current) = stack.pop() {
        if current == *new_parent {
            return true;
        }
        if visited.insert(current.clone()) {
            for edge in edges {
                if edge.parent_id == current {
                    stack.push(edge.child_id.clone());
                }
            }
        }
    }
    false
}
```

### API

Routes added inside the existing `api_routes` router (nested under `/v0/`):

```
POST   /repos/{owner}/{repo}/children/{child_owner}/{child_repo}
DELETE /repos/{owner}/{repo}/children/{child_owner}/{child_repo}
GET    /repos/{owner}/{repo}/children
GET    /repos/{owner}/{repo}/parents
```

The POST handler:
1. Reconstruct `RepoId` from path segments: `"{owner}/{repo}"`
2. Look up both repos by `repo_id` — 404 if either is missing
3. Reject self-edges (parent == child) — 422
4. Check for cycles — 422 with message "edge would create a cycle"
5. Insert edge — 200 (idempotent, no error on duplicate)

Response for children/parents: `ListResponse<RepoDto>` (same as existing repo list, reusing the DTO).

### RepoDto changes

```rust
pub struct RepoDto {
    pub object: String,
    pub path: String,
    pub name: String,
    pub repo_id: String,       // new — always present
    pub wave_count: u32,
    pub registered: bool,
    pub added_at: Option<String>,
}
```

### add_repo behavior change

Current flow: validate path → canonicalize → check `.git` → upsert.

New flow: validate path → canonicalize → check `.git` → derive RepoId from GitHub remote → reject if no remote (422) → check for existing repo with same RepoId but different path (409) → upsert.

The RepoId uniqueness constraint means you can't register two clones of the same repo. This is intentional — a portfolio should have exactly one local path per logical repo.

### Cascade on delete

When a repo is deleted (`DELETE /repos`), also delete all edges referencing its `repo_id`. This prevents orphaned edges.

### Python client + lfq

Add to `Client` class following existing patterns:

```python
def add_child(self, owner: str, repo: str, child_owner: str, child_repo: str) -> None:
    self._request_json("POST", f"/v0/repos/{owner}/{repo}/children/{child_owner}/{child_repo}")

def remove_child(self, owner: str, repo: str, child_owner: str, child_repo: str) -> None:
    self._request_json("DELETE", f"/v0/repos/{owner}/{repo}/children/{child_owner}/{child_repo}")

def list_children(self, owner: str, repo: str) -> list[Repo]:
    payload = self._request_json("GET", f"/v0/repos/{owner}/{repo}/children")
    return self._parse_model_list(payload, Repo)

def list_parents(self, owner: str, repo: str) -> list[Repo]:
    payload = self._request_json("GET", f"/v0/repos/{owner}/{repo}/parents")
    return self._parse_model_list(payload, Repo)
```

Add `repo_id: str` field to the `Repo` model.

Module-level wrappers in `api.py`. lfq CLI commands under `repos` subapp:

```
lfq repos children loopflowstudio/studio        # list children
lfq repos parents loopflowstudio/loopflow       # list parents
lfq repos add-child loopflowstudio/studio loopflowstudio/loopflow   # add child
lfq repos rm-child loopflowstudio/studio loopflowstudio/loopflow    # remove child
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| UUID-based repo identity | Stable, no remote required | Meaningless to humans, no natural derivation, requires lookup for everything |
| Path-based edges | No new identity concept | Breaks when repos move, not portable across machines |
| URL-encoded RepoId in path (`%2F`) | Single path segment | Ugly URLs, harder to read in logs and debug |
| Edges in repo config (`.lf/config.yaml`) | Distributed, lives with repo | Violates "lfd state, not repo config" principle. Can't manage from Concerto |
| `/edges/` in API URL | Generic graph concept | "add child" is the user-facing mental model, not "add edge" |

## Key decisions

**RepoId is derived, not assigned.** No user-provided identifiers. `github_repo_from_local()` already handles SSH, HTTPS, and SSH URI formats. This eliminates a class of "which ID do I use?" confusion.

**Path stays as primary key.** RepoId is an additional indexed column. Path is still needed for all filesystem operations. The alternative (RepoId as PK) would require changing every query that touches repos.

**GitHub remote required for all repos.** Not just new repos — the migration drops and recreates the table. All repos must have a GitHub remote to be re-added.

**No backwards compatibility.** Migration drops the repos table. Existing repos are re-added by the user. Clean slate.

**Cycle detection loads all edges into memory.** For portfolios under 100 repos, this is negligible. No need for persistent transitive closure tables or incremental algorithms.

**Idempotent edge creation.** `POST` on an existing edge succeeds silently. This simplifies client code and avoids "did it already exist?" checking.

## Scope

**In scope:**
- `RepoId` newtype and derivation from GitHub remote
- Migration: drop/recreate `repos` table with `repo_id`, create `repo_edges` table
- Store trait extension + SQLite + Postgres implementations
- HTTP endpoints: add/remove child, list children, list parents
- Cycle detection
- Cascade delete of edges when repo is removed
- `add_repo` behavior change (require GitHub remote, enforce uniqueness)
- Python client methods + `Repo` model update
- lfq CLI commands for child management
- Tests for cycle detection, edge CRUD, API handlers, and Python client

**Out of scope:**
- Context loading from related repos (wave item 02)
- Cross-repo stimulus (wave item 04)
- Concerto UI for edges (wave item 05)

## Done when

```bash
# Cycle detection rejects cycles
cargo test -p loopflow repo_edge

# API accepts child CRUD
curl -X POST localhost:4080/v0/repos/loopflowstudio/studio/children/loopflowstudio/loopflow
curl localhost:4080/v0/repos/loopflowstudio/studio/children
# → returns loopflow

curl localhost:4080/v0/repos/loopflowstudio/loopflow/parents
# → returns studio

# Python client works
lfq repos add-child loopflowstudio/studio loopflowstudio/loopflow
lfq repos children loopflowstudio/studio

# Edges persist across lfd restarts
```

Goals advanced: "Portfolio model supports directed acyclic edges between repos" and "Studio is the first consumer, with loopflow as child."

# cross-repo: portfolio DAG edges — design review

## What was implemented

Milestone 01 of the cross-repo wave: portfolio-level parent/child edges between repos.

**Rust (lfd):**
- `RepoId` newtype (`owner/repo` from GitHub remote, directory name fallback) with `FromStr`/`Display`
- `Repo` and `RepoEdge` domain types with full derive set
- Migration 021: `repos` table (path PK, repo_id unique index) and `repo_edges` table (composite PK, FK cascade)
- Store methods for both SQLite and Postgres: upsert/delete repos, add/remove/list edges, parent/child traversal
- Cycle detection via BFS before edge insertion
- HTTP routes: `POST/DELETE /v0/repos/:owner/:repo/children/:child_owner/:child_repo`, `GET .../children`, `GET .../parents`
- `GET /v0/repos` enriched: registered repos merged with wave-derived repos, each now carries `repo_id`, `registered`, `added_at`

**Python (lfq):**
- `Repo` model gains `repo_id` field
- Client methods: `add_child`, `remove_child`, `list_children`, `list_parents`
- CLI commands: `lfq repos children`, `lfq repos parents`, `lfq repos add-child`, `lfq repos rm-child`
- API module exports for programmatic access

**Wave plan:**
- `wave/cross-repo/` with README, yaml, and milestones 02–05

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| RepoId = `owner/repo` from GitHub remote | Stable identity across machines; matches GitHub API conventions | Full path (not portable), hash (opaque) |
| DAG in lfd, not repo config | Edges are portfolio state, not repo-local. Moving machines shouldn't lose structure. | `.lf/config.yaml` per repo — requires syncing N repos to change one edge |
| Cycle detection via BFS | Simple, correct, O(V+E) on a small graph. No library dependency. | Topological sort (same complexity, more code), DFS (equivalent but BFS reads clearer) |
| FK CASCADE on repo_edges | Deleting a repo auto-cleans edges. No manual cleanup in `delete_repo`. | Manual DELETE before repo removal — more code, easy to forget |
| Directory name fallback for repo_id | Non-GitHub repos still get a usable identity | Refuse to register — too restrictive for local-only workflows |

## How it fits together

```
Concerto / lfq
    │
    ▼
lfd HTTP routes (/v0/repos/...)
    │
    ▼
Store trait (mod.rs dispatcher)
    ├── SqliteStore
    └── PostgresStore
    │
    ▼
Domain types (RepoId, Repo, RepoEdge)
```

The repo list endpoint merges two sources: explicitly registered repos (from the `repos` table) and wave-derived repos (from `waves` table paths). Unregistered repos appear with `registered: false` and a best-effort `repo_id` derived from git remote or directory name.

Edges connect `repo_id` values, not paths — so edges survive repo relocation.

## Risks and bottlenecks

- **Cycle detection scales linearly** with graph size. Fine for portfolios (tens of repos), but no guard against a degenerate case with thousands of edges. Not a concern now.
- **GitHub remote required for stable identity.** Repos without a GitHub remote fall back to directory name, which is fragile across machines. The wave README documents this constraint.
- **No cross-repo session wiring yet.** This milestone delivers the graph — milestones 02–04 deliver context loading, commit splitting, and stimulus routing that consume it.

## What's not included

- Context loading from child repos into parent sessions (milestone 02)
- Cross-repo commit splitting (milestone 03)
- Cross-repo stimulus/listen (milestone 04)
- Concerto UI for edge management (milestone 05)
- Transactional cross-repo operations (explicitly out of scope per wave README)

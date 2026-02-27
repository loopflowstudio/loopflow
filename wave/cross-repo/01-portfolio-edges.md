# 01: Portfolio DAG Edges

## What to build

Extend lfd's portfolio model from a flat list of repos to a DAG with directed parent→child edges.

## Data structures

```rust
// Extend existing portfolio/repo types
message RepoEdge {
    string parent_repo_id = 1;
    string child_repo_id = 2;
}

// Portfolio gains edges
message Portfolio {
    repeated Repo repos = 1;
    repeated RepoEdge edges = 2;
}
```

## Key functions

- `add_edge(parent_id, child_id)` — Add directed edge. Reject if it would create a cycle.
- `remove_edge(parent_id, child_id)` — Remove edge.
- `get_children(repo_id) -> Vec<Repo>` — List children of a repo.
- `has_cycle(edges) -> bool` — Cycle detection (DFS from child, check if parent is reachable).

## API

```
POST   /repos/{id}/children/{child_id}   — add edge
DELETE /repos/{id}/children/{child_id}   — remove edge
GET    /repos/{id}/children              — list children
```

## Constraints

- Cycle detection must run on every edge addition. A cycle means a repo is both ancestor and descendant of another — reject with clear error.
- Edges reference repos already in the portfolio. Can't add an edge to a repo that isn't tracked.
- Persisted alongside existing portfolio state.

## Done when

- lfd API accepts add/remove edge calls
- Cycle detection rejects cycles
- `get_children` returns correct children after edge mutations
- Edges persist across lfd restarts

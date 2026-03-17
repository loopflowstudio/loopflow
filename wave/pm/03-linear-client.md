---
pm_id: '1213718054769167'
---
# 03: Linear client

**Finish line:** `LinearClient` implements `PmProvider` over the Linear GraphQL API. All trait methods work against a real Linear workspace.

The shared PM seam (`PmProvider` trait in `pm/mod.rs`), Asana reference client (`pm/asana.rs`), Linear credential storage (`Provider::Linear` in `provider_auth.rs`, `lfq auth linear`), and repo/global PM config parsing (`LinearConfig` in `engine::config`) already exist on main. This item adds only the GraphQL transport, Linear-specific project/issue mapping, and the state lookup needed to make `complete_item` mean "done" in a real workspace.

## What to build

Rust GraphQL client using `reqwest` (raw POST to `https://api.linear.app/graphql`). No codegen — hand-written queries for the small surface area. Mirror the Asana client shape: `LinearClient::new(token, config)`, provider-local request/response types, server-directed 429 retry handling, and focused mock-server tests.

### Queries/mutations needed

| Trait method | Linear operation |
|-------------|-----------------|
| `create_project` | `projectCreate` mutation |
| `list_items` | `project.issues` query (with pagination) |
| `create_item` | `issueCreate` mutation |
| `update_item` | `issueUpdate` mutation |
| `complete_item` | `issueUpdate` (set state to "Done") |
| `comment` | `commentCreate` mutation |

### Auth and config

- `Authorization: {api_key}` header. Key comes from `Provider::Linear` in the encrypted provider-token store (`lfq auth linear` already stores it).
- Read the default team from `.lf/config.yaml` `linear.team` through `LinearConfig` in `engine::config`. The config type, CLI auth command, and HTTP auth route already exist on main.
- Keep remote IDs as strings even when Linear returns UUIDs or project keys.

### Markdown

Linear descriptions are native markdown — direct content transfer, no conversion needed.

### Completion state

Linear issues have workflow states (Backlog, Todo, In Progress, Done, Cancelled). `complete_item` needs to find the completed state for the team and set it. Query `workflowStates` filtered by completed type instead of hard-coding a state name.

### Rate limiting

Linear returns `429` with rate limit headers. Same retry/backoff pattern as Asana client.

## Constraints

- No GraphQL client crate — `reqwest` POST with `serde_json` query bodies.
- Linear pagination uses cursor-based pagination — handle projects with many issues.
- Fail clearly when `linear.team` is required but missing.
- Put the implementation in `rust/loopflow/src/lfd/pm/linear.rs` beside Asana instead of growing `pm/mod.rs`.

## Done when

- `LinearClient::new(...)` constructs a client from stored auth + config
- All six trait methods work against the Linear API
- `cargo test -p loopflow pm::linear` passes with mock HTTP server
- Integration test against real Linear documents the manual verification

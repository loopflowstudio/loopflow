# 03: Linear client

**Finish line:** `LinearClient` implements `PmProvider` over the Linear GraphQL API. All trait methods work against a real Linear workspace.

The shared PM seam and Linear credential storage already exist. This item adds the GraphQL transport, Linear-specific project/issue mapping, and the state lookup needed to make `complete_item` mean "done" in a real workspace.

## What to build

Rust GraphQL client using `reqwest` (raw POST to `https://api.linear.app/graphql`). No codegen — hand-written queries for the small surface area.

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

- `Authorization: {api_key}` header. Key comes from the existing encrypted provider-token store.
- Read the default team from `.lf/config.yaml` `linear.team` through `engine::config` for project and issue creation paths.
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

## Done when

- `LinearClient::new(...)` constructs a client from stored auth + config
- All six trait methods work against the Linear API
- `cargo test` passes with mock HTTP server
- Integration test against real Linear documents the manual verification

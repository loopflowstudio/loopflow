# 03: Linear client

**Finish line:** `LinearClient` implements `PmProvider` over the Linear GraphQL API. All trait methods work against a real Linear workspace.

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

### Auth

`Authorization: {api_key}` header. Key retrieved from provider credential storage.

### Markdown

Linear descriptions are native markdown — direct content transfer, no conversion needed.

### Completion state

Linear issues have workflow states (Backlog, Todo, In Progress, Done, Cancelled). `complete_item` needs to find the "Done" state for the team and set it. Query `workflowStates` filtered by `type: "completed"`.

### Rate limiting

Linear returns `429` with rate limit headers. Same retry/backoff pattern as Asana client.

## Constraints

- No GraphQL client crate — `reqwest` POST with `serde_json` query bodies
- Linear pagination uses cursor-based pagination — handle for projects with many issues
- Team ID needed for issue creation — read from `.lf/config.yaml` linear.team

## Done when

- `LinearClient::new(api_key, team_id)` constructs a client
- All six trait methods work against the Linear API
- `cargo test` passes with mock HTTP server
- Integration test against real Linear documents the manual verification

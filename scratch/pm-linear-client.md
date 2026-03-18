---
pm_id: '1213718054769167'
---
# Linear Client

## Problem

Loopflow's PM integration has Asana but not Linear. Teams using Linear can't sync wave items to their tracker. The `PmProvider` trait, `LinearConfig`, credential storage, and CLI auth all exist — the missing piece is the GraphQL transport that maps Linear's API to the six trait methods.

Linear is native markdown and uses workflow states for completion, making it a cleaner fit than Asana in some ways — but GraphQL and cursor-based pagination are different enough from Asana's REST API that this isn't a copy-paste job.

## Approach

Single file: `rust/loopflow/src/lfd/pm/linear.rs`. Mirror the Asana client's shape — `LinearClient::new(token, team_id)`, `with_base_url` for tests, `graphql` with 429 retry, then the six `PmProvider` methods. All GraphQL queries are inline string constants, no codegen.

The client takes a resolved `String` team ID, not `LinearConfig`. The caller resolves `linear.team` from config before construction — same pattern as Asana resolving workspace/team before hitting the API. This keeps the client free of config-resolution logic and makes the team requirement visible in the constructor signature.

### GraphQL transport

One method: `graphql<T>(&self, query: &str, variables: Value) -> PmResult<T>`. POST to `https://api.linear.app/graphql` with `{"query": ..., "variables": ...}` body. Auth header: `Authorization: {api_key}` (not Bearer — Linear uses bare API keys). Parse response, check for `errors` array, extract `data`.

**Error rule:** if the `errors` array is non-empty, return `PmError` with the first error message — regardless of whether `data` is also present. GraphQL allows partial success (both `data` and `errors`), but our operations are single mutations/queries where partial data would be misleading. Simple rule, correct for this surface area.

### Trait method mapping

| Trait method | GraphQL operation | Notes |
|-------------|-------------------|-------|
| `create_project` | `projectCreate` mutation | Uses `team_id` from constructor. |
| `list_items` | `project { issues }` query | Cursor-based pagination via `endCursor` / `hasNextPage`. |
| `create_item` | `issueCreate` mutation | Pass `team_id`, `projectId`, `title`, `description`. |
| `update_item` | `issueUpdate` mutation | Map `name` → `title`, `description` → `description`. Skip if empty. |
| `complete_item` | `workflowStates` query + `issueUpdate` | Query team's workflow states, find first with `type: "completed"`, set `stateId`. If no completed state found, return `PmError`. Cache nothing — one extra query per completion is fine for correctness. |
| `comment` | `commentCreate` mutation | `issueId` + `body` (markdown). |

### Key differences from Asana

1. **Auth header format.** Linear uses `Authorization: {api_key}`, not Bearer token.
2. **Team is resolved before construction.** `LinearClient::new` takes a resolved team ID string. The caller reads `linear.team` from config and fails if missing — the client itself never touches config. Asana resolves workspace/team lazily inside the client; Linear pushes that up.
3. **Pagination.** Cursor-based (`endCursor`/`hasNextPage`) instead of offset-based. Same loop pattern, different field names.
4. **Completion requires state lookup.** Asana sets `completed: true`. Linear needs to find the team's "completed" workflow state ID and set it via `issueUpdate(stateId:)`. Query `workflowStates(filter: { team: { id: { eq: teamId } }, type: { eq: "completed" } })`.
5. **Error shape.** GraphQL returns 200 with `errors` array for most failures. Only transport/auth failures return non-200 status codes. Must check both.
6. **Field naming.** Linear uses `title` where Asana uses `name`. `description` is the same. Map in the trait implementation, not in types.

### Response types

Provider-local serde types, not shared. Follow the Asana pattern:

```
GraphqlResponse<T> { data: Option<T>, errors: Option<Vec<GraphqlError>> }
GraphqlError { message: String }
ProjectCreatePayload { project_create: ProjectNode }
IssueCreatePayload { issue_create: IssueNode }
IssueUpdatePayload { issue_update: IssueNode }
CommentCreatePayload { comment_create: CommentNode }
ProjectNode { id: String }
IssueNode { id: String, title: String, description: String, ... }
IssuesConnection { nodes: Vec<IssueNode>, page_info: PageInfo }
PageInfo { has_next_page: bool, end_cursor: Option<String> }
WorkflowStateNode { id: String, name: String, r#type: String }
```

### Rate limiting

Same pattern as Asana: retry up to 3 times on 429, respect `Retry-After` header, fall back to 60s. Linear's rate limit is lower (400 req/min vs Asana's 1500) but we're not doing bulk operations in this client — individual trait method calls won't hit it.

### Test strategy

Reuse the Asana test infrastructure pattern: axum mock server with `spawn_test_server`, `CapturedRequest`, `QueuedResponse`. The test server code is identical in structure — both record requests and play back queued responses. Tests cover:

- `create_project` sends correct mutation with team ID
- `list_items` paginates and assigns rank by response order
- `create_item`, `update_item`, `complete_item`, `comment` map to correct operations
- `complete_item` queries workflow states before updating
- `complete_item` fails clearly when no completed state exists for the team
- `update_item` skips empty updates
- Rate limit retry works
- GraphQL error messages surface clearly (errors array overrides data)

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| GraphQL codegen (`cynic`, `graphql-client`) | Type safety from schema, auto-generated types | Adds a build dependency for 6 queries. Hand-written queries are clearer for this surface area. |
| Cache workflow states | Avoid extra query on `complete_item` | Adds staleness risk and state management for a call that happens once per issue lifecycle. One extra roundtrip is fine. |
| Share test server infra between Asana and Linear | Less duplication | Coupling test infrastructure across providers creates a reason to change both when only one changes. Copy the pattern, not the code. |

## Key decisions

**Bare API key auth, not Bearer.** Linear's API uses `Authorization: {api_key}` without the Bearer prefix. The Asana client uses `.bearer_auth()`. Linear will set the header manually.

**Team ID in constructor, not config.** `LinearClient::new(token, team_id)` takes a resolved team ID. The caller (ops command, bootstrap CLI) resolves `linear.team` from config and fails with an actionable message if missing. The client never reads config directly.

**No state caching for `complete_item`.** Each `complete_item` call queries workflow states fresh. This is correct-by-default and the operation is infrequent enough that the extra roundtrip doesn't matter.

**Duplicate test server pattern.** The mock server setup in Asana tests (~100 lines) will be duplicated in Linear tests rather than extracted. The two providers have different request/response shapes and will evolve independently.

## Scope

- In scope: `LinearClient` implementing all six `PmProvider` methods, GraphQL transport with retry, mock-server tests, `pub mod linear;` in `pm/mod.rs`
- Out of scope: Caching, batching, webhook support, label/priority sync, project templates, real Linear integration test script (document manual verification steps in a comment instead)

## Done when

```bash
cargo test -p loopflow pm::linear
```

All tests pass. The six trait methods send correct GraphQL queries/mutations to the mock server. Error paths (missing team, GraphQL errors, rate limits) are covered.

Wave goal advanced: "Linear client implements `PmProvider`, matching the Asana seam and test coverage."

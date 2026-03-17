---
pm_id: '1213718054769167'
---
# Linear Client

## Problem

Loopflow's PM integration has Asana but not Linear. Teams using Linear can't sync wave items to their tracker. The `PmProvider` trait, `LinearConfig`, credential storage, and CLI auth all exist — the missing piece is the GraphQL transport that maps Linear's API to the six trait methods.

Linear is native markdown and uses workflow states for completion, making it a cleaner fit than Asana in some ways — but GraphQL and cursor-based pagination are different enough from Asana's REST API that this isn't a copy-paste job.

## Approach

Single file: `rust/loopflow/src/lfd/pm/linear.rs`. Mirror the Asana client's shape exactly — `LinearClient::new(token, config)`, `with_base_url` for tests, `send_json` with 429 retry, then the six `PmProvider` methods. All GraphQL queries are inline string constants, no codegen.

### GraphQL transport

One method: `graphql<T>(&self, query: &str, variables: Value) -> PmResult<T>`. POST to `https://api.linear.app/graphql` with `{"query": ..., "variables": ...}` body. Auth header: `Authorization: {api_key}` (not Bearer — Linear uses bare API keys). Parse response, check for `errors` array, extract `data`.

### Trait method mapping

| Trait method | GraphQL operation | Notes |
|-------------|-------------------|-------|
| `create_project` | `projectCreate` mutation | Requires `teamId` from config. Fail if `linear.team` missing. |
| `list_items` | `project { issues }` query | Cursor-based pagination via `endCursor` / `hasNextPage`. |
| `create_item` | `issueCreate` mutation | Pass `teamId`, `projectId`, `title`, `description`. |
| `update_item` | `issueUpdate` mutation | Map `name` → `title`, `description` → `description`. Skip if empty. |
| `complete_item` | `workflowStates` query + `issueUpdate` | Query team's workflow states, find the one with `type: "completed"`, set `stateId`. Cache nothing — one extra query per completion is fine for correctness. |
| `comment` | `commentCreate` mutation | `issueId` + `body` (markdown). |

### Key differences from Asana

1. **Auth header format.** Linear uses `Authorization: {api_key}`, not Bearer token.
2. **Team is required upfront.** Linear needs `teamId` for project and issue creation. Asana resolves workspace/team lazily. Linear fails fast if `linear.team` is missing from config.
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
- `create_project` fails when team config is missing
- `list_items` paginates and assigns rank by response order
- `create_item`, `update_item`, `complete_item`, `comment` map to correct operations
- `complete_item` queries workflow states before updating
- `update_item` skips empty updates
- Rate limit retry works
- GraphQL error messages surface clearly

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| GraphQL codegen (`cynic`, `graphql-client`) | Type safety from schema, auto-generated types | Adds a build dependency for 6 queries. Hand-written queries are clearer for this surface area. |
| Cache workflow states | Avoid extra query on `complete_item` | Adds staleness risk and state management for a call that happens once per issue lifecycle. One extra roundtrip is fine. |
| Share test server infra between Asana and Linear | Less duplication | Coupling test infrastructure across providers creates a reason to change both when only one changes. Copy the pattern, not the code. |

## Key decisions

**Bare API key auth, not Bearer.** Linear's API uses `Authorization: {api_key}` without the Bearer prefix. The Asana client uses `.bearer_auth()`. Linear will set the header manually.

**Fail fast on missing team.** Unlike Asana where workspace/team can be auto-detected, Linear requires a team ID for most mutations. `create_project` and `create_item` return `PmError` immediately if `linear.team` is unset, with an actionable message pointing to `.lf/config.yaml`.

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

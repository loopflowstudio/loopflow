# Stripe-style v1 HTTP API for lfd

## Goal
Adopt a Stripe-inspired API surface for lfd: `/v1` base path, resource-oriented URLs, consistent JSON payloads, Stripe-like list envelopes, structured errors, pagination primitives, idempotency keys, and expandable fields. Align Rust lfd + Swift client and provide a migration plan.

## Non-goals
- Designing a future `/v2` namespace (we just want v1 to be future-proof).
- Reworking business logic, scheduler, or storage internals beyond what API responses require.
- Full auth/permissions redesign (keep current auth middleware behavior).

## Proposed API shape (v1)

### Base path
- Single namespace lives at `/v1` (no parallel `/api` or legacy root namespace).
- Keep `/health` and `/status` at root for daemon availability checks.

### Waves
```
GET    /v1/waves
POST   /v1/waves
GET    /v1/waves/{id}
PATCH  /v1/waves/{id}
DELETE /v1/waves/{id}
POST   /v1/waves/{id}/run
POST   /v1/waves/{id}/stop
POST   /v1/waves/{id}/land
```

### Wave runs
```
GET /v1/wave_runs
GET /v1/waves/{id}/runs
```

#### WaveRun fields (draft)
- `id`, `wave_id`, `iteration`, `step_index`, `status`, `local_worktree`, `remote_branch`, `started_at`, `ended_at`, `error`, `flow_parents`
- `local_worktree`: human-friendly local path/name (may omit the fully-unique suffix).\n+- `remote_branch`: canonical fully-unique branch name pushed to origin.

### Worktrees (if needed by Concerto)
```
GET /v1/worktrees
```

### List responses (Stripe-style)
```
{
  "object": "list",
  "data": [ ... ],
  "has_more": false
}
```

### Single resources
```
{
  "id": "...",
  "object": "wave",
  ...
}
```

### Error payload
```
{
  "error": {
    "type": "invalid_request_error",
    "message": "...",
    "param": "repo"
  }
}
```
Use Stripe-style names when relevant; keep the set minimal.

### Pagination
- Support `limit`, `starting_after`, `ending_before` query params.
- Return `has_more` for list endpoints.

### Idempotency
- Honor `Idempotency-Key` for POST/DELETE to prevent duplicate side effects.
- Document exact replay semantics (when we retry vs return stored response).

### Expandable fields
- `expand[]=active_run` or `expand[]=recent_steps` on GET endpoints.
- Default responses should be minimal, with expansion for heavier fields.

### Core vs expanded fields (draft)
- Treat `created_at` as expandable (not core).
- Represent paused as a `status=paused` enum value (no separate `paused` boolean).
- GitHub/PR metadata handled client-side for now (mirror helpers in Python + Swift).
- PR-related expansions are TBD until worktree landing behavior is clearer.

### Version header
- Use `Loopflow-Version` for client-requested response shaping (even while v1 is unlocked).

### JSON-only
- v1 is JSON-only for request + response payloads.

## Mapping to current lfd/Concerto

### Current lfd (Rust)
- Routes mounted at `/api/v1` (needs `/v1`)
- Wave responses are wrappers (`WaveViewDto`, `ListWavesResponse`) without Stripe list envelope.
- No `/wave_runs` list endpoint.
- No structured error payload.
- No idempotency handling.

### Current Concerto (Swift)
- Expects root `/waves`, `/status` etc.
- Expects `{ ok: true, result: { ... } }` wrapper.
- Expects wave fields not provided by Rust lfd (`pr_url`, `staleness`, `recent_steps`, etc.)

## Migration plan
1. **Move lfd routes to `/v1`** (no parallel `/api/v1`).
2. **Adopt Stripe-style payloads** in `/v1` routes (no wrapper, list envelope, error object).
3. **Add wave_runs list endpoints** + map to Concerto expectations.
4. **Update Concerto** to call `/v1` and parse new response shapes.

## Open questions
- Which fields are mandatory in `wave` and `wave_run` for Concerto UI today?
- Do we want `object` strings like `wave`, `wave_run`, `worktree` everywhere?
- Should expansions include GitHub metadata (PR URLs, status) or should Concerto enrich client-side?
- Should we provide a short-lived compatibility shim in Concerto or only the new API?
- When do we plan to lock v1 semantics (and what compatibility promises do we want once it’s locked)?

## Implementation notes
- Rust: add DTOs for Stripe list envelope + error type. Replace `{ ok, result }` parsing in Swift.
- Swift: introduce a single API base path constant (`/v1`) and decode Stripe shapes.
- Testing: add API contract tests in Rust + Swift decoding tests for the new envelope.

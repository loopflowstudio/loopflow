# Branch review: jack-heart.pm.20260317_1653

## What was implemented

Linear `PmProvider` — full GraphQL client covering project creation, issue CRUD, pagination, completion-state lookup, rate-limit retry, and comment posting. Shared PM infrastructure extracted from the Asana client so both providers use the same retry logic, test server, and noop-update filtering.

## Key choices

- **Per-provider ID fields** (`asana_id`, `linear_id`) replace the generic `pm_id`. Each provider's ID lives in its own frontmatter key, so a wave item can be linked to both providers simultaneously. `RoadmapItemFrontmatter::id_for(provider)` and `set_id(provider, id)` provide the dispatch.

- **Shared `PmTextUpdate`** filters rank-only updates at the trait boundary. Both Asana and Linear `update_item` implementations use `update.text_update()` to skip API calls when only rank changed (rank is a local concern, not pushed to providers).

- **Shared test server** moved from `asana::tests` to `pm::test_server` as a `pub(crate)` module. Both provider test suites now share the same `spawn()` / `json_response()` / `response()` helpers.

- **`RATE_LIMIT_RETRIES` and `retry_after_delay`** promoted to `pm::mod.rs` constants/functions. Asana and Linear use identical retry semantics.

- **Linear auth is API-key-based** (`lf ops auth configure linear`), not OAuth. Simpler than Asana's flow — just store the key.

- **Test isolation for HOME** — `EnvGuard::with_home()` and `with_isolated_home()` prevent developer `~/.lf/config.yaml` from leaking into integration tests.

## How it fits together

`PmProvider` trait in `pm/mod.rs` defines the contract. `AsanaClient` and `LinearClient` implement it. The `PmProviderKind` enum dispatches. `RoadmapItemDocument` handles frontmatter serialization with provider-specific ID fields. Shared retry and test infra live in the parent module.

## Risks and bottlenecks

- Linear's GraphQL API uses `String!` for IDs (not `ID!`), which works but may need adjustment if Linear changes their schema.
- `resolve_team_id` creates a "Loopflow" team if none exists — same pattern as Asana. Could surprise users who don't expect team creation.
- No caching of `completed_state_id` — each `complete_item` call makes two API requests. Acceptable for wave-scale usage but would need caching at higher volumes.

## What's not included

- Notion provider (tracked as wave item 08)
- CLI bootstrap commands (`lf pm bootstrap`, `lf pm link`) — next wave item (04)
- PM sync flow and steps — wave items 05-07
- Actual `lf ops export` integration with `LinearClient` (ops/pm.rs is WIP, unstaged)

## Validation

- `cargo fmt --check` -- clean
- `cargo clippy -p loopflow -- -D warnings` -- clean
- `cargo test -p loopflow pm::linear` -- 9 passed
- `cargo test -p loopflow pm::asana` -- 10 passed
- `cargo test -p loopflow pm::tests` -- 12 passed
- `cargo test -p loopflow --test config_tests` -- 22 passed
- `cargo test -p loopflow --test land_tests` -- 7 passed
- `cargo test -p loopflow --test pr_tests` -- 5 passed
- `uv run pytest python/tests/ -q` -- 115 passed

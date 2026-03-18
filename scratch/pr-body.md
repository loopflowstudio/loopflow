## Try it!

```bash
cargo test -p loopflow pm::linear
cargo test -p loopflow pm::asana
cargo test -p loopflow pm::tests
cargo test -p loopflow --test config_tests
cargo test -p loopflow --test land_tests
cargo test -p loopflow --test pr_tests
uv run pytest python/tests/ -q
```

What to look for:
- Linear's `PmProvider` exercises project creation, pagination, completion-state lookup, rate-limit retry, and GraphQL error handling — all against a shared in-process test server.
- Asana tests migrated to the same shared test server with no behavior changes.
- Config/PR/land integration tests now pass regardless of developer `~/.lf/config.yaml`.

## Intent

Ship the Linear PM provider and consolidate the shared seam so both Asana and Linear implementations use identical retry logic, test infrastructure, and noop-update filtering. This completes the client layer needed before CLI bootstrap and sync commands.

## Assumptions

- Linear API uses `String!` for entity IDs in GraphQL variables (not `ID!`).
- Linear team auto-creation (named "Loopflow") matches the Asana pattern and is acceptable UX.
- Per-provider frontmatter fields (`asana_id`, `linear_id`) are preferred over a generic `pm_id` to support multi-provider linking.

## Key decisions

- **`PmTextUpdate` as a filter type** — rank-only updates are a local concern; providers should never see them. The `text_update()` method on `PmItemUpdate` returns `None` when only rank is set, letting both providers skip the API call uniformly.
- **Shared `test_server` module** — extracted to `pm::test_server` as `pub(crate)` instead of duplicating across provider test files.
- **`EnvGuard::with_home()`** — solves test flakiness from developer `~/.lf/config.yaml` leaking into integration tests that check default config values.

## Not included

- CLI bootstrap/link commands (wave item 04)
- PM sync flow and steps (wave items 05-07)
- Notion provider (wave item 08)

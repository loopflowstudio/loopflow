## Try it!

```bash
cargo test -p loopflow pm::linear
cargo test -p loopflow --test config_tests
cargo test -p loopflow --test land_tests
cargo test -p loopflow --test pr_tests
uv run pytest python/tests/ -q
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift
```

What to look for:
- Linear's `PmProvider` implementation exercises project creation, pagination, completion-state lookup, rate-limit retry, and GraphQL error handling.
- Local auth docs now include `lf ops auth configure linear` for PM flows that run through `lf ops`.
- Config/PR/land integration tests now pass even when the developer machine has a personal `~/.lf/config.yaml`.

## Intent

This branch finishes the PM foundation for loopflow while carrying forward the auth and attention-queue infrastructure it builds on. It establishes a provider-agnostic PM seam, adds concrete Asana and Linear clients plus credential flows, and wires that into wave config, roadmap frontmatter, export plumbing, and user-facing auth surfaces so PM-backed workflows can ship without provider-specific side paths.

## Assumptions

- Real Asana OAuth still depends on valid `ASANA_CLIENT_ID` / `ASANA_CLIENT_SECRET` configuration.
- Real Linear project creation still depends on a resolved team ID and a stored `LINEAR_API_KEY`.
- Swift validation assumes the existing GhosttyKit binary artifact is available; package tests currently tolerate its umbrella-header warnings.
- Reviewers can evaluate this branch by subsystem: auth/storage, PM transport, export/config plumbing, and attention surfaces.

## Key decisions

- Keep `PmProvider` and roadmap/frontmatter parsing in one shared module, with provider-specific HTTP/GraphQL logic isolated to `asana.rs` and `linear.rs`.
- Treat PM API keys as non-metered credentials in both CLI status and UX copy.
- Resolve Linear team IDs before constructing `LinearClient` instead of baking config lookups into the client.
- Fix environment-sensitive tests by isolating `HOME` inside integration tests rather than weakening production config behavior.
- Let Python auth polling follow provider expiry when available, which keeps Asana/browser flows aligned with the server-side timeout contract.

## Not included

- Jira/Notion support.
- Bidirectional live sync, caching, batching, or webhook-driven PM updates.
- Full Xcode UI-test coverage in this gate pass.

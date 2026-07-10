# Gate review: native Linear planning and token continuity

## What was implemented

`lf pm` now maps waves to Linear Initiatives, projects to native Linear
Projects, and tasks to Issues. `lf pm init` seeds the hierarchy from local
project Markdown, migrates legacy labeled tasks without guessing ambiguous
destinations, and keeps the legacy handle until every task is assigned.

Linear OAuth now persists the non-secret PKCE client ID, refreshes access tokens
20 minutes before expiry in both PM commands and the daemon refresher, preserves
rotated or omitted refresh tokens, and gives an actionable reconnect error only
after an expired token can no longer refresh. `lf pm show` renders a deterministic
aligned table and fetches project task lists concurrently.

## Key choices

- Linear is authoritative for project inventory, definitions, KRs, and tasks.
  `wave/<wave>/projects/` is a generated offline cache and one-time seed.
- Project slugs derive from Linear Project names. Empty or duplicate derived
  slugs are hard drift errors rather than silently ambiguous routing.
- Legacy tasks migrate only when exactly one recognized `project:<slug>` label
  selects a destination. Zero or multiple matches leave the task in place and
  retain `pm.linear_project` for a later retry.
- Initialization writes a transient `pm.linear_seed_pending` marker after
  creating an Initiative and before seeding Projects. A failed run resumes
  missing Projects on the same Initiative; normal completion removes the marker.
- GraphQL operations use Linear's schema-native `String` identifier types.
  Unit coverage pins these declarations because mocked servers cannot perform
  GraphQL variable validation.

## How it fits together

`WavePmConfig` stores the Initiative handle and migration state. The PM ops
layer resolves that context, refreshes the stored credential if due, validates
the native project set, and delegates GraphQL calls to `LinearClient`. Sync
projects Linear content back into prompt-readable Markdown; task commands read
and mutate Issues directly.

## Risks and bottlenecks

- `lf pm init` mutates Linear and GOAL frontmatter. It is restart-safe, but a
  live production migration should still be observed and its diagnostics read.
- Existing OAuth rows without a stored client ID need configured legacy client
  credentials or one `lf auth linear` reconnect.
- Project names are identity-bearing because they derive CLI slugs. Renames can
  change the slug and therefore the cache filename and task command input.
- `pm status` and `pm sync` still read project issue lists sequentially; the
  interactive `pm show` path is concurrent.

## What's not included

- No automatic production migration during install or startup; `lf pm init` is
  explicit per wave.
- No CLI mutation for Project definition/KR content; edit it in Linear and pull
  the cache with `lf pm sync`.
- No automated KR evidence or provider-neutral Initiative abstraction beyond
  the current Linear implementation.

## Validation

- `cargo run -q -p loopflow --bin lf -- pm sync --plan` completed against the
  live Linear API and reported the three expected legacy wave migrations. No
  remote mutation was performed.
- `cargo test -p loopflow pm`: 45 PM-related tests passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `uv run python scripts/test.py --all`: all six suites passed — Python, Rust
  (1,317 tests at that checkpoint), website, Swift, E2E, and Loopflow app compile.
- A final `cargo nextest run --all` revalidated the completed Rust tree after
  the restart-safety and concurrency polish: 1,318 passed, 3 skipped.

## Wave alignment

The change advances Developer Efficiency's credential-expiry KR and Technical
Architecture's 1:1 domain-map KR. Explicit migration, resumable seeding, strict
slug validation, and sanitized refresh failures keep the release-stability risk
visible rather than silently splitting local and Linear state.

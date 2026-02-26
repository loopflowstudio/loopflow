# Contract hardening review (Pass 2)

## What was implemented

- Added explicit prompt pipeline stage types in `engine/prompt.rs`:
  - `GatheredContext(PromptComponents)`
  - `BudgetedContext(PromptComponents, ContextBreakdown)`
  - `RenderedPrompt(String)`
- Updated prompt call sites (`engine/launch.rs`, `ops/lint.rs`, `ops/rebase.rs`, `bin/lf-prompt.rs`, integration tests) to follow gather → budget → render ordering.
- Split Docker executor into lifecycle modules:
  - `docker/mod.rs` (orchestration + shared constants)
  - `docker/image.rs`, `docker/workspace.rs`, `docker/recovery.rs`, `docker/io.rs`
- Promoted Docker metadata conventions to shared constants (managed labels, kind labels, container/volume prefixes) and replaced scattered literals with those constants.
- Added Docker invariant tests in `docker/tests.rs` for:
  - label/prefix contracts,
  - startup rehydration/orphan cleanup,
  - workspace branch-resolution precedence,
  - mount/volume/credential behavior.
- Hardened SQL catalog invariants in `lfd/store/catalog.rs`:
  - Query enum coverage check,
  - renderability for SQLite/Postgres,
  - contiguous placeholder checks for templates and overrides.
- Added/updated golden prompt fixtures (`tests/goldens/builtin_{implement,review,debug}.{yaml,md}`) and parity tests.
- Kept store backend dispatch centralized in `lfd/store/mod.rs` and documented intentional explicit matching at the dispatch point.

## Key choices

- **Newtypes instead of struct explosion**: enforce prompt stage ordering without copying fields into new per-stage structs.
- **Test-time contract checks instead of codegen**: SQL catalog remains source-of-truth; focused tests catch drift with lower complexity.
- **Constants over implicit string conventions**: Docker recovery/workspace/image now share a single metadata vocabulary.
- **Keep store dispatch explicit**: avoided macro indirection for now; centralized `match` surface remains easy to audit.

## How it fits together

Prompt assembly now has compiler-visible stage boundaries, so call sites must pass through gather/budget/render in order. Docker lifecycle code is decomposed by concern with shared contract constants in `mod.rs`, and tests pin the recovery/mount/label invariants. Store SQL safety is reinforced by catalog-wide invariant tests that validate completeness and placeholder correctness across both supported dialects.

## Risks and bottlenecks

- `engine/prompt.rs` is still large; stage contracts are explicit, but file-size/concentration risk remains.
- Store dispatch remains verbose; it is intentional, but still high-churn when adding methods.
- Docker startup recovery still depends on runtime Docker availability for full confidence; unit coverage is strong but not a substitute for end-to-end daemon restart scenarios.

## What’s not included

- No new flow language or trigger model changes.
- No HTTP API redesign.
- No full store backend abstraction rewrite (dispatch macro/adapter extraction deferred).
- No LLM-as-judge quality eval harness; quality guardrail here is deterministic golden snapshots.

## Wave alignment

This branch directly advances `wave/infra` goals:
- **Maintain compactness**: docker lifecycle responsibilities moved out of one mega-file.
- **Reduce duplication/drift**: Docker metadata constants + SQL catalog invariants.
- **Stabilize extension seams**: prompt stage contracts and explicit backend dispatch point.

Known infra risks were addressed in-scope:
- Metadata convention drift in Docker recovery: mitigated with constants + invariant tests.
- Contract drift in prompt/store paths: mitigated with stage types + catalog/golden tests.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow prompt`
- `cargo test -p loopflow store`
- `cargo test -p loopflow recovery`
- `cargo test -p loopflow api_subset`
- `cargo test -p loopflow golden_prompt`

All passed locally.

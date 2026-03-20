# PM priority rename + init rework — review

## What was implemented

- Replaced the roadmap sync model's exact global ordering with shared semantic priority buckets: `Urgent`, `High`, `Medium`, and `Low`, stored locally as `1-` through `4-` file prefixes.
- Moved provider-specific priority translation to the edges: Asana uses semantic custom-field labels, Linear uses its native numeric priorities, and local sync logic keeps using `PriorityBucket`.
- Simplified `lf ops pm init` so it creates a fresh PM team/projects for every wave, clears stale provider item IDs before bootstrap, and writes the new provider/project linkage back to wave config and roadmap files.
- Hardened PM sync helpers with shared provider-ID clearing, shared Linear priority conversion helpers, and a fix for stripping markdown H1 headings from imported project descriptions when the body starts with blank lines.

## Key choices

- **Keep `PriorityBucket` as the shared model.** Provider adapters translate into Asana/Linear-native representations instead of leaking provider semantics into ingest, prompts, or sync orchestration.
- **Prefer fresh bootstrap over remote matching in `pm init`.** The command now creates new PM state instead of trying to reconcile with existing remote projects. That makes init deterministic and easier to reason about, at the cost of being explicitly destructive to old local provider IDs.
- **Preserve legacy numbered files as fallback only.** `ingest` prefers bucketed files first, but old numbered files still parse during transition so existing waves do not break mid-migration.
- **Centralize tiny cross-provider helpers.** `semantic_label`, `linear_value` / `from_linear_value`, and `clear_id` remove repeated edge logic and make adapter behavior easier to review.

## How it fits together

`docs/wave-authoring.md`, builtin prompts, and `ops/ingest.rs` now all teach and consume the same four-bucket roadmap model. `lfd/pm/mod.rs` owns the shared `PriorityBucket` and frontmatter helpers, while `asana.rs` and `linear.rs` translate that model into each provider's native API. `ops/pm.rs` remains the orchestration layer for init/pull/status/sync and now bootstraps fresh PM projects while keeping local roadmap files as the durable source of truth.

## Risks and bottlenecks

- Asana priority mapping still depends on semantically recognizable option labels (`Urgent` / `High` / `Medium` / `Low`). Unexpected custom labels will not map cleanly.
- Within-bucket ordering is still local filename order, not a shared provider-wide ordering contract.
- `lf ops pm init` intentionally creates fresh remote state and clears stored provider IDs first; reviewers should validate that this reset behavior matches the intended operator workflow.
- Notion is still out of scope, so this branch proves the shared model against Asana/Linear only.

## What's not included

- Notion integration or README/supporting-doc sync.
- Arbitrary/custom priority taxonomies beyond the four shared semantic buckets.
- A new deterministic ordering rule within the same priority bucket.
- Automatic migration that rewrites every legacy numbered roadmap file to bucketed names.

## Validation

Passed locally:

- `cargo fmt --check`
- `cargo clippy -p loopflow -- -D warnings`
- `cargo test -p loopflow`

Additional spot check:

- `rg -n "Urgent|High|Medium|Low|PriorityBucket|from_semantic_label" docs/wave-authoring.md rust/loopflow/src/engine/builtins rust/loopflow/src/ops/ingest.rs rust/loopflow/src/lfd/pm rust/loopflow/src/ops/pm.rs`

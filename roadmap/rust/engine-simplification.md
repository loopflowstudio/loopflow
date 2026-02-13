# Engine Simplification

Reduce dead surface area and duplication in the engine crate. Three priorities ordered by impact.

## 1. Unify prompt building in engine

CLI `build_prompt()` and daemon `build_step_prompt()` follow the same 8-step pipeline with ~70% overlap. `merge_directions()` is copy-pasted across two files.

**Action**: Extract `engine::prompt::build_step_prompt()` with a `StepPromptOpts` struct. Move `merge_directions()` to engine. CLI and daemon become thin wrappers.

## 2. Delete dead config surface

Config fields that parse but never execute:
- `include_loopflow_doc` (always true, never checked)
- `budgets` / `BudgetConfig` (trimming uses flat `max_tokens`)
- `context` and `exclude` (never consumed)
- `push`, `pr`, `land` (ops flags, never consumed outside tests)
- `ide` / `IdeConfig` (defined but unused)
- `summary_tokens` / `SummaryConfig` (summaries are daemon-managed)

**Action**: Delete these fields and their parsing/merging/default code.

## 3. Simplify context-gathering toggles

`lfdocs`, `diff_files`, `diff` in global config are always-true booleans. Step frontmatter already supports opting out (`diff_files: false`).

**Action**: Remove global toggles, keep step-level overrides.

## Deferred

- **Step/flow auto-wrapping**: Design change, not simplification. Needs own design doc.
- **Direction parse-time validation**: Additive. Bundle with `merge_directions()` move.
- **Fork worktree path unification**: Cosmetic divergence. Unify when adding next executor backend.

## Done (this branch)

- ~~**`AutopruneConfig` custom deserializer**~~: Replaced 54-line custom Visitor with `#[serde(untagged)]` enum.
- ~~**Sequential-to-parallel fork cleanup**~~: CLI uses `thread::scope`, daemon uses `tokio::task::JoinSet`.

## Design decisions

- **Hardcoded synthesize step name**: Forks always run `"synthesize"` after branches complete. Previously configurable via `fork.synthesize` field — removed to simplify the fork data model. Flows needing custom post-fork steps should use explicit step sequencing instead.

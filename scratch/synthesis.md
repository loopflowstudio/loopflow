# Synthesis

## Perspectives

**Infra-engineer** focused on structural duplication between CLI and daemon: prompt building duplicated across `run.rs` and `executor.rs`, fork worktree path conventions diverging, `merge_directions()` copy-pasted. Also flagged dead config fields (`push`, `pr`, `land`, `ide`, `include_loopflow_doc`, `budgets`). Emphasis on making the engine layer the single source of truth for shared operations.

**Designer** focused on the gap between configuration surface and runtime behavior: config fields that parse but never execute (`context`, `exclude`, `budgets`, `include_loopflow_doc`), creating false promises. Identified the summary infrastructure as partially built — config, trimming, formatting, and daemon injection all exist, but CLI's `gather_context()` returns empty. Emphasis on coherence between what the system advertises and what it delivers.

**Product-engineer** focused on abstraction boundaries: auto-wrapping steps as flows creates a heuristic unwrapping layer, direction names travel separately from direction content causing late validation failures, config booleans (`lfdocs`, `diff_files`, `diff`) add API surface for decisions that are already made. Emphasis on making the type system enforce invariants rather than runtime checks.

## Agreements

All three perspectives converge on these:

1. **Prompt building is duplicated and should be unified.** CLI `build_prompt()` and daemon `build_step_prompt()` follow the same 8-step pipeline with 3 small parameter differences. `merge_directions()` is identically copy-pasted in two files. This is the strongest signal — every perspective identified it independently, with consistent specifics (line numbers, divergence points, shared function signatures).

2. **Dead config fields should be deleted.** All three identified config fields that parse but never execute. The specific fields vary slightly by perspective, but the core set overlaps: `include_loopflow_doc`, `budgets`/`BudgetConfig`, and the boolean context toggles. The principle is the same: delete what nothing reads, add it back when consuming code exists.

3. **Core abstractions are well-designed.** Flow expansion, agent executor trait, context assembly pipeline, stream parsing — all three called these out as aligned with product intent. The simplification work is at the edges, not the core.

## Tensions

**What to do about summaries.** Designer identified the summary infrastructure as half-built across config, prompt, and daemon — config defines it, trimming handles it, formatting renders it, daemon generates and injects it, but CLI returns empty. The tension: is this dead code to remove, or an incomplete feature to finish? The designer suggests two paths: (a) implement summary loading in `gather_context()` for CLI, or (b) strip summary config from engine and keep it daemon-only. The infra-engineer didn't flag summaries. The product-engineer didn't either.

*Resolution*: Summaries are inherently a daemon concern — they require an internal agent call to generate, which the CLI doesn't have. Keep `PromptComponents.summaries` (the daemon injects into it), but delete `SummaryConfig` from engine config and the unused `summary_tokens` field. The daemon manages its own summary config. This matches the actual architecture: engine assembles, daemon enriches.

**Auto-wrapping steps as flows.** Product-engineer identified `load_flow()` auto-wrapping step names as a source of heuristic complexity in expansion. Neither other perspective raised this. The concern is valid — there's a 20-line conditional block in expansion devoted to undoing the auto-wrap — but the blast radius of changing `load_flow()` semantics is broader than a reduce pass should attempt. This is a design change, not a simplification.

*Resolution*: Note for future design work, not for this reduce pass. The existing test coverage (7 expansion tests including ambiguous name resolution) means the heuristic works reliably today.

**Direction validation timing.** Product-engineer wants parse-time validation of direction names. Infra-engineer wants fork conventions unified. Neither is wrong, but they optimize different things. Parse-time validation catches typos earlier; unified conventions prevent divergence.

*Resolution*: Both. `merge_directions()` moves to engine (infra-engineer's point). Direction name validation at flow load time is a small addition with clear value (product-engineer's point). These are complementary, not competing.

## Synthesis

Three simplification opportunities, ordered by impact and confidence:

### 1. Unify prompt building in engine

Extract `engine::prompt::build_step_prompt()` that takes a `StepPromptOpts` struct. Move `merge_directions()` to `engine::fork` (or `engine::prompt`). CLI and daemon become thin wrappers that construct opts from their respective contexts (CLI flags vs wave store).

This is the highest-leverage change. Every perspective identified it. The duplication is concrete (two 70-80 line functions with ~70% overlap, one copy-pasted helper). The risk is low — the two implementations already do the same thing, so unification is mostly mechanical.

### 2. Delete dead config surface

Remove config fields that parse but never execute:
- `include_loopflow_doc` (always true, never checked)
- `budgets` / `BudgetConfig` (trimming uses a flat `max_tokens`, not per-section budgets)
- `context` and `exclude` (never consumed by `gather_context()` or `should_exclude()`)
- `push`, `pr`, `land` (ops flags, never consumed outside tests)
- `ide` / `IdeConfig` (defined but unused)
- `summary_tokens` and `SummaryConfig` from engine config (summaries are daemon-managed)

This is the simplest change — pure deletion. Each field removed shrinks config parsing, merging, defaults, and tests. The principle: config should reflect what the system actually does, not what it might someday do.

### 3. Simplify context-gathering boolean toggles

Make context gathering include everything by default. Remove `lfdocs`, `diff_files`, `diff` from global config. Step frontmatter already supports opting out (`diff_files: false` in debug.md). This is the right mechanism — step-level overrides, not global toggles that every caller mechanically passes through.

`GatherContextOpts` loses 3-4 boolean fields. Every call site that passes `config.lfdocs` etc. simplifies. The product decision (always include context) is expressed structurally rather than through defaults-that-nobody-changes.

### Not in this pass

- **Step/flow auto-wrapping**: Real concern, but a design change. Needs its own design doc.
- **Direction parse-time validation**: Good idea, but additive (new feature, not simplification). Bundle with the `merge_directions()` move.
- **Fork worktree path unification**: Low urgency — both conventions work, divergence is cosmetic. Unify when adding the next executor backend.
- **`AutopruneConfig` custom deserializer**: 50 lines of compat code for a config format. Low priority but easy to clean up.

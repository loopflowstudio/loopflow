# Simplification Opportunities

## Product intent
Loopflow is a small set of built-in steps, flows, and directions that assemble context and run a coding agent with minimal ceremony. The product wants a single, coherent “source of truth” for steps/flows and a straightforward path from CLI input → context → prompt → agent run.

## Opportunity 1: Single source of truth for built-in steps/flows metadata
**Misalignment**: Built-in step/flow content lives in `loopflow-engine` (embedded files), but the CLI keeps its own lists, categories, and descriptions (`rust/lf/src/discovery.rs`). The product intent is “built-ins are canonical everywhere,” yet the architecture maintains duplicate registries.
**Symptom**: `BUILTIN_CATEGORIES`, `builtin_descriptions()`, and `builtin_steps()` manually mirror names already embedded in `loopflow-engine::builtins`, so adding/removing a step requires touching multiple places and risks drift.
**Realignment**: Introduce a single manifest (YAML/JSON or frontmatter-based metadata) embedded alongside the built-in step files and expose it from `loopflow-engine` for listing, categories, and descriptions. The CLI should query that manifest instead of maintaining its own list.
**Cascade**: Removes duplicated lists, eliminates mismatches between help output and actual steps, simplifies step discovery, and reduces the need for ad-hoc category logic in the CLI.

## Opportunity 2: Collapse context configuration into a single “context policy”
**Misalignment**: The product wants “run a step with the right context,” but the architecture splits this across CLI flags, config defaults, and `GatherContextOpts` with multiple booleans (`diff`, `diff_files`, `lfdocs`, `clipboard`, `area`, etc.), plus separate trimming/cleanup steps.
**Symptom**: `rust/lf/src/commands/run.rs` builds `GatherContextOpts` by merging CLI + config + step metadata, then later calls `drop_native_instruction_docs` and `trim_context_with_breakdown` as separate phases. This makes the “what context do we include?” policy spread across several functions.
**Realignment**: Create a single `ContextPolicy`/`ContextRequest` type in `loopflow-engine` that owns defaults, merges overrides, and performs any required pruning in one place. The CLI should pass a minimal set of user intents (e.g., `--clipboard`, `--area`, `--no-diff`) and let the engine decide the final policy.
**Cascade**: Fewer flags to reason about, fewer translation layers, simpler CLI code, and a clearer boundary between “intent” and “implementation.”

## Opportunity 3: Make interactive behavior a property of steps, not config
**Misalignment**: Steps already declare `interactive` in frontmatter, but the CLI also checks `config.interactive` and other conditions to decide whether to run interactively. The product intent is that the step defines its interaction mode.
**Symptom**: `run.rs` combines `cli.interactive`, `cli.batch`, step metadata, config lists, and “no step + no inline” cases to infer interactive mode, leading to a tangled conditional.
**Realignment**: Treat step frontmatter as the authoritative interactive flag, with explicit CLI overrides (`--interactive`, `--batch`) only. Remove `config.interactive` as a parallel source and define a single rule: step decides unless CLI overrides.
**Cascade**: Simplifies run-mode logic, reduces config surface area, and makes step behavior more predictable.

## Aligned areas
- `loopflow-engine` now embeds built-in step/flow/direction content and loads it consistently, which matches the “works everywhere” product story.
- Prompt assembly and context trimming are centralized in `loopflow-engine::prompt`, keeping most LLM-facing logic in one place.

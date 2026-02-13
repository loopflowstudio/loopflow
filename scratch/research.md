# Execution Stack: Architecture & Simplification

Research and reduction analysis for the loopflow execution stack. Covers the engine/CLI/daemon architecture and identifies simplification priorities.

## Architecture

Three clean layers:

```
engine/  (shared library: flow loading, agent commands, context assembly, worktree ops)
   ↑              ↑
   │              │
lf/            lfd/
(CLI)          (daemon)
```

Neither CLI nor daemon imports the other. Both consume `engine::` types. Python client (`lfq`) and Swift app (Concerto) talk to the daemon via REST API.

### Engine

14 modules. Core responsibilities:
- **flow.rs** — Load YAML flows, parse into `Flow`/`FlowItem` trees, expand into `ConcreteItem` sequences, drive execution via `next_action()`
- **fork.rs** — Fork utilities: worktree path naming, direction merging, manifest serialization, cleanup
- **prompt.rs** — Context assembly: step/direction/diff/clipboard/area docs, token counting (tiktoken), budget-based trimming with eviction priority
- **agent.rs** — Command building for 4 agent backends (Claude, Codex, Gemini, OpenCode), process management
- **stream.rs** — Parse streaming JSON from all 4 agents into unified `StreamEvent` types
- **config.rs** — Layered config (global + repo YAML), model parsing, context budget defaults
- **worktree.rs / worktrees.rs** — Git worktree lifecycle, branch naming, cleanup
- **builtins.rs** — `include_str!()` embedded steps (18), flows (12), directions (7), ops prompts (5)

### Data flow

```
YAML (steps/*.md, flows/*.yaml)
  → load_flow() → expand_flow() → Vec<ConcreteItem>
  → next_action(items, index) → FlowAction
  → match FlowAction:
      RunStep      → gather_context() → trim → format_prompt() → agent
      Fork(All)    → create worktrees → parallel execution → synthesize → cleanup
      WaitInteract → pause for user → then RunStep
      Complete     → finalize
```

Context assembly trims to budget with eviction order: area → summaries → docs → diff files → diff → clipboard.

### Key abstractions

**Flow model**: `Flow` → `Vec<FlowItem>` (Step, Fork, FlowRef). Expand recursively (max depth 5, cycle detection) into `Vec<ConcreteItem>`.

**AgentExecutor trait**: 4 methods (`run`, `terminate`, `recover_startup`, `cleanup_wave`). Two implementations: `LocalProcessExecutor` (subprocess) and `DockerExecutor` (containers).

**Config layering**: Global (`~/.lf/config.yaml`) + repo (`.lf/config.yaml`). Daemon has separate config (`~/.lf/lfd.yaml`).

## Simplification priorities

Three perspectives (infra-engineer, designer, product-engineer) converged on these priorities, ordered by impact:

### 1. Unify prompt building in engine

CLI `build_prompt()` and daemon `build_step_prompt()` follow the same 8-step pipeline with ~70% overlap. `merge_directions()` is copy-pasted across two files.

**Action**: Extract `engine::prompt::build_step_prompt()` with a `StepPromptOpts` struct. Move `merge_directions()` to engine. CLI and daemon become thin wrappers.

### 2. Delete dead config surface

Config fields that parse but never execute:
- `include_loopflow_doc` (always true, never checked)
- `budgets` / `BudgetConfig` (trimming uses flat `max_tokens`)
- `context` and `exclude` (never consumed)
- `push`, `pr`, `land` (ops flags, never consumed outside tests)
- `ide` / `IdeConfig` (defined but unused)
- `summary_tokens` / `SummaryConfig` (summaries are daemon-managed)

**Action**: Delete these fields and their parsing/merging/default code.

### 3. Simplify context-gathering toggles

`lfdocs`, `diff_files`, `diff` in global config are always-true booleans. Step frontmatter already supports opting out (`diff_files: false`).

**Action**: Remove global toggles, keep step-level overrides.

### Deferred

- **Step/flow auto-wrapping**: Design change, not simplification. Needs own design doc.
- **Direction parse-time validation**: Additive. Bundle with `merge_directions()` move.
- **Fork worktree path unification**: Cosmetic divergence. Unify when adding next executor backend.

## Observations

### Strengths

- Core abstractions (flow expansion, executor trait, context pipeline, stream parsing) are well-designed
- 253 `#[test]` functions, tests assert on behavior not mock wiring
- Flow expansion tests are thorough (7 cases: parsing, nesting, cycle detection, ambiguous names)
- Consistent `thiserror`/`anyhow` error hierarchy, derives follow style guide

### Test gaps

No integration tests for end-to-end fork execution (parallel worktree creation → thread spawning → result aggregation → cleanup on failure). Fork unit tests cover helpers only. This is the most critical untested code path.

### Known issues

- `executor.rs` is 3693 lines mixing 10+ responsibilities. Roadmap calls out splitting this pre-EC2 executor.
- `gather_context()` blocks the async daemon. Fix: wrap in `tokio::spawn_blocking()`.
- Fork task abort may leak scheduler slots if `scheduler.release()` doesn't run.
- `prepared_key` tracking has no TTL — stale code possible after remote pushes between daemon runs.
- Orphaned fork worktrees can accumulate silently (removal errors logged but not propagated).

## Open questions

- Should `ForkSelect::One`/`Prompt` be supported in CLI, or is the daemon the right boundary for interactive fork execution?
- Is `prepared_key` sufficient without TTL for long-running daemons?
- Should daemon startup detect and clean orphaned fork worktrees?

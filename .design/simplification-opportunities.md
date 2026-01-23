# Simplification Opportunities

## Product intent

Loopflow assembles context and prompts for AI coding agents. Users run steps (markdown prompts), optionally chained as flows. The daemon (`lfd`) automates this for background agents. The core value: reusable prompts that work across Claude Code, Codex, and Gemini CLI.

## Opportunity 1: Naming inconsistency between CLI and models

**Misalignment**: The execution model design doc renamed Run→FlowRun and Session→StepRun, but legacy names persist throughout the codebase. The CLI still uses "task" instead of "step" in some places, and "session" instead of "step_run".

**Symptoms**:
- `collector.py` parameter named `task` but represents a step name
- `session_id` used where `step_run_id` is more accurate
- Mixed terminology in logs, comments, and user-facing messages
- Protocol layer uses `sessions.*` methods (e.g., `sessions.end`) while models are `StepRun`
- `lf/models.py` sends `{"session_id": step_run_id}` — parameter is renamed but protocol key isn't
- Database migration renames table to `step_runs` but protocol hasn't caught up

**Realignment**: Use consistent terminology everywhere:
- "step" = a prompt definition (markdown file)
- "step_run" = execution of a step
- "flow" = sequence of steps
- "flow_run" = execution of a flow

**Cascade**: Clearer mental model for users, simpler documentation, fewer translation comments.

## Opportunity 2: Config sprawl across multiple concerns

**Misalignment**: `Config` class mixes unrelated concerns—IDE settings, git behavior, token budgets, skill sources, branch naming, autoprune. These config keys serve different features that rarely interact.

**Symptoms**:
- 25+ config keys in a single flat model
- `AutopruneConfig`, `IdeConfig`, `WorkConfig`, `SummaryConfig` nested but others inline
- Keys like `yolo`, `chrome`, `push`, `pr`, `land` are execution-time flags, not configuration
- Deprecated `include_tests_for` still supported with warnings

**Realignment**: Separate config by domain:
1. **Prompt assembly** (context, exclude, summaries, skill_sources)
2. **Execution** (agent_model, yolo, interactive)
3. **Git workflow** (push, pr, land, auto_rebase)
4. **Daemon settings** (autoprune, branch_names)

**Cascade**: Each config section becomes independently testable, easier to document, and flags that belong in CLI args move out of config entirely.

## Opportunity 3: Dual entry points with shared state

**Misalignment**: `lf` and `lfd` are separate CLIs but share models, context gathering, and execution logic. The daemon reaches deep into `lf` internals (`format_prompt`, `gather_prompt_components`, flow loading). This coupling means changes to prompt assembly risk breaking daemon execution.

**Symptoms**:
- `runner.py` imports from both `lf.context`, `lf.flows`, `lf.voices`
- `collector.py` handles both interactive and streaming modes with different codepaths
- Same step execution code appears in `lf/run.py` and `lfd/execution/runner.py`
- `lf` CLI can run standalone but daemon builds its own prompt assembly

**Realignment**: Clear boundary at "assemble prompt, then execute":
1. `lf` owns prompt assembly and CLI UX
2. `lfd` owns scheduling, process management, and background execution
3. Shared: step loading, flow definitions, output logging

**Cascade**: Daemon becomes a scheduler that calls `lf` primitives rather than reimplementing them. The Rust daemon exploration (`.docs/rust-lfd.md`) already identified this as the right boundary.

## Aligned areas

**Step discovery and loading**: Clean search order (repo → global → builtin), consistent frontmatter parsing, external skill resolution. The `gather_step()` function is well-structured.

**Flow definition**: Pydantic models for FlowStep/FlowDef are clean, support fork/join/choose patterns, and serialize reliably. The DAG model matches product intent.

**Token management**: `trim_prompt_components()` is a clear solution to context limits—greedy drop of largest components with explicit tracking. The token counting abstraction works.

**Agent model in lfd**: The recent refactor to unified Agent model (combining loop/subscription/schedule) simplifies the daemon's state management. This is good realignment.

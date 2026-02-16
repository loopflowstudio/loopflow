# Simplification Opportunities

## Product intent

Loopflow chains coding agents into repeatable workflows. Users define steps (prompts), compose them into flows, and run them as autonomous waves. The product is a workflow engine for LLM agents—not a general-purpose orchestrator. It should feel like `make` for coding agents: declare what you want, run it, get working code.

## Opportunity 1: Executor is an entire application inside one file

**Misalignment**: The wave executor (`lfd/executor.rs`, 4043 lines, 134KB) handles seven distinct concerns—agent execution, Docker infrastructure, workspace preparation, flow orchestration, fork management, summary caching, and CI sidecar operations. The product's execution model is simple (run steps in sequence, optionally fork), but the implementation reflects the accumulated complexity of every deployment mode and edge case packed into one place.

**Symptom**: The same "build prompt → create agent → run → store result" pattern is implemented four times independently: `run_step()`, `execute_ci_fix_agent()`, `run_internal_summarize()`, and the fork branch closure. The fork closure captures 25+ variables. Docker-specific logic (~2500 lines) dwarfs the local executor (~100 lines) despite both implementing the same trait.

**Realignment**: Extract the repeated agent-launch cycle into a single helper: `fn launch_agent(step, prompt, worktree) -> AgentResult`. Move Docker workspace management into its own module (`lfd/docker/`). Move fork execution into a dedicated function that takes the launch helper as a parameter. The executor becomes a state machine that dispatches to extracted concerns rather than implementing everything inline.

**Cascade**: Fork execution becomes testable in isolation. Docker and local executors diverge cleanly (Docker gets its volume/image/workspace module, local stays trivial). The summary system decouples from the executor. CI sidecar logic becomes a separate concern that plugs into the execution lifecycle rather than being woven through it.

## Opportunity 2: Dual-backend store abstracts away a decision that's already made

**Misalignment**: The store layer maintains a `RunStore` trait with 47 methods, implemented identically for SQLite (824 lines) and PostgreSQL (970 lines). The SQL queries are the same—only parameter placeholder syntax differs (`?1` vs `$1`). The product defaults to SQLite; Postgres is opt-in via environment variable. The architecture is generic where the product is specific.

**Symptom**: The sync `RunStore` trait forces the async Postgres implementation into `block_on()` wrappers. Dead code markers (`#[allow(dead_code)]`) on `PostgresStore::connect()` and `PostgresStore::migrate()` confess the mismatch. Developers must maintain 47 identical method stubs across two files. Total store layer: 2,696 lines.

**Realignment**: Make SQLite the only store. If Postgres is needed later, it can be re-added behind a feature flag with a query builder or macro that eliminates the duplication. Alternatively, keep both but use a macro to generate implementations from shared SQL, reducing the 1,800 lines of duplication to ~200 lines of macro invocations.

**Cascade**: Deleting the Postgres impl removes ~970 lines and the `RunStore` trait abstraction (~460 lines). The remaining SQLite store becomes direct and concrete—no trait indirection, no async/sync impedance mismatch. The daemon binary simplifies (no runtime backend selection). Migration management halves.

## Opportunity 3: Context assembly handles too many gathering strategies in one file

**Misalignment**: `engine/prompt.rs` (80KB) handles token counting, five different document-gathering strategies, diff tiering, context trimming, prompt formatting (three variants), and prompt logging. The product concept is straightforward: assemble relevant context for an agent. But the implementation treats every source (scratch docs, area docs, diff, clipboard, explicit files) as a unique path with its own reading, filtering, and deduplication logic.

**Symptom**: Four nearly-identical directory-walking functions (`gather_md_files`, `gather_files`, `gather_dir_files`, `gather_all_text_files`). File reading inlined in five separate places instead of going through a shared path. Three format functions (`format_prompt`, `format_context_prompt`, `format_task_prompt`) that share 80% of their logic. The `Document` struct is overloaded—it represents repo docs, diff files, area READMEs, and explicit files without clear type distinction.

**Realignment**: Unify document gathering into one function that takes a source specification (directory + filter) and returns documents. Replace the three format functions with a single `format_prompt(mode: PromptMode)` that switches on what to include. Give Document a typed source enum (`Source::Scratch`, `Source::Area`, `Source::Diff`, `Source::Explicit`) instead of a loose category string.

**Cascade**: New context sources (e.g., wave summaries, which are currently a TODO) would plug into the unified gathering path instead of requiring a new bespoke function. Token trimming could use the typed source enum for priority ordering instead of hardcoded field-by-field logic. The three format variants collapse into one.

## Aligned areas

**Step/flow/direction discovery**: The lookup chain (skills → repo → global → builtins) is clean, well-ordered, and matches how users think about customization. User definitions silently override builtins—correct behavior, no complexity.

**CLI argument handling**: The argument reordering trick (`lf debug -c` → `lf -c debug`) is a good product decision that lets users think in terms of "step first, flags second." The implementation is contained and doesn't leak complexity.

**Agent backend abstraction**: Supporting Claude, Codex, Gemini, and OpenCode costs ~400 lines of backend-specific code each (reasonable). The real complexity is in stream parsing (1,014 lines for four JSON formats), but this is inherent—different agents emit different formats. The abstraction earns its keep.

**Flow expansion**: The step → flow → concrete plan pipeline is clean. Cycle detection, max depth, and fork handling are well-contained in `engine/flow.rs`. The data structures map directly to user concepts.

**Git operations layering**: The split between `engine/git.rs` (primitives) and `ops/` (workflows) is conceptually sound. Minor duplication exists (`has_staged_changes` duplicated twice, some PR query functions reimplemented), but the boundary is clear and the complexity matches the actual product complexity of git workflow automation.

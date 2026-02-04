# Review: promptpass

Native context loading for all backends (Claude, Codex, Gemini).

## What was implemented

**Python:**
- Split prompt assembly into `format_context_prompt()` (system) and `format_task_prompt()` (user message)
- `build_*_command()` functions accept `context_file` parameter
- `get_model_env()` accepts `gemini_context_file` for `GEMINI_SYSTEM_MD` env var
- `execute_step()` writes context file and passes it to all backends

**Rust:**
- Added `context_file` to `LaunchConfig` struct
- `build_claude_command()` uses `--append-system-prompt-file`
- `build_codex_command()` uses `-c model_instructions_file="..."`
- `launch_agent()` sets `GEMINI_SYSTEM_MD` env var for Gemini
- `format_context_prompt()` and `format_task_prompt()` split prompt assembly
- `write_prompt_log()` writes to `.lf/log/` and ensures `.lf/log/` is in repo `.gitignore`
- Bundled `LOOPFLOW.md` in binary via `include_str!`
- Wave-scoped roadmap filtering (only `roadmap/<wave>/` included when wave is set)
- Interactive mode message for `run_mode=interactive`

## Key choices

1. **Context/task split**: Context goes to system prompt via native mechanisms, task goes as CLI argument. Keeps input history clean.

2. **Backend-specific loading**:
   - Claude: `--append-system-prompt-file` (appends to system prompt)
   - Codex: `-c model_instructions_file="..."` (replaces AGENTS.md)
   - Gemini: `GEMINI_SYSTEM_MD` env var (replaces system prompt)

3. **Inline prompts skip context file**: Short inline prompts don't need the split; they use full prompt directly.

4. **Wave filtering**: `roadmap/` contents excluded unless wave is set. When set, only `roadmap/<wave>/` is included.

5. **Bundled LOOPFLOW.md**: Embedded in Rust binary via `include_str!` macro. Always included in context.

## How it fits together

```
gather_context() → PromptComponents
    ↓
format_context_prompt() → context string (written to .lf/log/*.context.md)
format_task_prompt()    → task string (CLI argument)
    ↓
launch_agent(task, LaunchConfig { context_file: ... })
    ↓
Backend-specific command:
- Claude: claude --append-system-prompt-file /path/to/context.md "task"
- Codex: codex exec -c model_instructions_file="/path/to/context.md" "task"
- Gemini: GEMINI_SYSTEM_MD=/path/to/context.md gemini "task"
```

## Risks and bottlenecks

- **Codex model_instructions_file**: Replaces AGENTS.md entirely. If a repo relies on AGENTS.md discovery, it loses that content. Mitigated: our context includes root *.md files via `gather_lfdocs`.

- **Gemini GEMINI_SYSTEM_MD**: Replaces system prompt entirely. Same mitigation as Codex.

- **Large context files**: Very large prompts could still cause issues, but the split at least keeps input history clean. Token budgets in `ContextConfig` provide trimming.

## What's not included

- lfd executor doesn't use context_file split (uses full prompt, consistent with existing behavior)
- No automated tests for actual CLI invocation (would require mocking subprocesses)
- No dry-run flag to preview commands without execution

## Test coverage

- `rust/loopflow-engine/tests/context_tests.rs`: 25 tests covering wave filtering, prompt formatting, context/task split
- `rust/loopflow-engine/src/prompt.rs`: Unit tests for `write_prompt_log`, `format_context_prompt`, `format_task_prompt`
- Python tests: 679 passed (existing coverage, no new Python tests added)

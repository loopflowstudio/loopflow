# promptpass Review

## What was implemented

Native context loading for coding agents (Claude, Codex, Gemini) via their native system prompt mechanisms instead of passing everything as a user prompt. This keeps the input history clean and leverages each agent's preferred method for system instructions.

Key changes:

1. **Context/task separation** — Split prompts into two files:
   - `<step>.context.md` — Loopflow docs, repo docs, directions (system context)
   - `<step>.md` — The task prompt (user input)

2. **Agent-specific loading** — Each agent loads context via its native mechanism:
   - Claude: `--append-system-prompt-file`
   - Codex: `-c model_instructions_file="..."`
   - Gemini: `GEMINI_SYSTEM_MD` environment variable

3. **Unified CLI entry point** — Merged `step`, `inline`, and `chat` commands into a single `run` function that handles all three cases.

## Key choices

**Why native context loading?**
- Cleaner input history in agent UIs
- Better separation of concerns (context vs task)
- More efficient for long-running sessions where context doesn't change

**Why bundle LOOPFLOW.md?**
- Always available regardless of repo state
- Consistent behavior across all invocations
- No file system lookups for core documentation

**Why split context and task?**
- Context (docs, directions, loopflow system) is static for a session
- Task (step content, inline prompt) is the actual work
- Agents can cache/optimize system context separately

## How it fits together

```
gather_context() → PromptComponents
    ↓
format_context_prompt() → context file (system instructions)
format_task_prompt() → task prompt (user message)
    ↓
launch_agent() → passes context_file via LaunchConfig
    ↓
build_*_command() → translates to agent-specific flags/env
```

The `LaunchConfig.context_file` field carries the path through the system. Each agent's command builder knows how to use it for that agent.

## Risks and bottlenecks

1. **File system dependency** — Context files written to `.lf/log/`. If disk is full or permissions wrong, fails. Mitigated by existing gitignore setup.

2. **Agent CLI compatibility** — Assumes specific flag names for each agent. If agents change their CLIs, need updates. Mitigated by version pinning in practice.

3. **Gemini env var** — Uses `GEMINI_SYSTEM_MD` which may conflict with user's environment. Low risk since it's set per-invocation.

## What's not included

- **Web client mode** — Still copies full prompt to clipboard (no context separation)
- **Streaming context updates** — Context is static for the session
- **Python CLI parity** — Python `lf` doesn't use these new Rust paths yet (uses its own context.py)

## CI fix in this session

Added missing `message` field to `GatherContextOpts` and `context_file` field to `LaunchConfig` in `python.rs`. These fields were added to the Rust structs but the PyO3 bindings weren't updated.

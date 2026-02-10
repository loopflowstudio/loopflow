---
status: todo
phase: 3
---
# OpenCode Context Injection

Enable loopflow to inject assembled context (LOOPFLOW.md, area docs, directions, diffs) into opencode sessions.

## Current

Each agent has a different context injection mechanism:
- **Claude**: `--append-system-prompt-file <path>` CLI flag
- **Codex**: `-c model_instructions_file="<path>"` config override
- **Gemini**: `GEMINI_SYSTEM_MD=<path>` environment variable

Loopflow writes assembled context to a temp file, then passes it via the agent's mechanism.

## OpenCode's Options

OpenCode has three mechanisms for injecting instructions:

1. **`AGENTS.md`** — reads from project root automatically. Also falls back to `CLAUDE.md`. But this is a file on disk, not a temp file we control per-run.

2. **`instructions` array in `opencode.json`** — points to file paths/globs. But modifying `opencode.json` per-run is invasive.

3. **`OPENCODE_CONFIG_CONTENT` env var** — inline JSON config that merges at runtime. This is the winner: we can inject an instructions reference to our temp file without touching any config files.

## Build

In `launch_agent`, when `backend == "opencode"`, merge the context file path into the `OPENCODE_CONFIG_CONTENT` env var (which PR 01 already uses for permission auto-approve):

```rust
if backend == "opencode" {
    let mut config_content = serde_json::Map::new();

    // Auto-approve permissions in auto mode
    if config.auto {
        config_content.insert(
            "permission".into(),
            serde_json::Value::String("allow".into()),
        );
    }

    // Inject context file as an instruction source
    if let Some(ref context_file) = config.context_file {
        config_content.insert(
            "instructions".into(),
            serde_json::json!([context_file.to_string_lossy()]),
        );
    }

    if !config_content.is_empty() {
        cmd.env(
            "OPENCODE_CONFIG_CONTENT",
            serde_json::Value::Object(config_content).to_string(),
        );
    }
}
```

This produces:
```json
{"permission":"allow","instructions":["/tmp/lf-context-abc123.md"]}
```

OpenCode loads the instructions array and includes the file contents in the LLM's context, alongside any user-defined instructions from their `opencode.json`.

### Update PR 01

PR 01 sets `OPENCODE_CONFIG_CONTENT` to just `{"permission":"allow"}`. This PR refactors that into the unified env var builder above. The permission logic moves from a hardcoded string to the `serde_json::Map` approach.

## Constraints

- Don't modify opencode's `opencode.json` — that's user-owned config
- `OPENCODE_CONFIG_CONTENT` merges with existing config, doesn't replace it
- Context file is a temp file cleaned up after the run
- The `instructions` array accepts relative or absolute paths — use absolute for temp files
- This approach lets users keep their own instructions in `opencode.json` while loopflow adds its context alongside

## Done when

```bash
# With opencode installed:
lf implement --agent opencode  # in a repo with .lf/ context
# Verify the assembled context reaches the opencode session
```

Unit test: verify `OPENCODE_CONFIG_CONTENT` env var contains both `permission` and `instructions` keys when both `auto` and `context_file` are set.

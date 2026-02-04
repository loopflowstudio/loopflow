# Promptpass: Native Context Loading for All Backends

## Problem

Currently, only Claude Code gets clean prompt delivery:
- Context loaded via `--append-system-prompt-file` (into system prompt)
- Task passed as CLI argument (user message)
- Input history shows only the task, not pages of context

Codex and Gemini get the full prompt crammed into the CLI argument:
```python
# execution.py:166-167
if supports_context_file:
    cli_prompt = task_prompt
else:
    cli_prompt = format_prompt(params.components)  # Full blob
```

This pollutes input history and buries the actual task in context.

## Solution

All three backends have native file-based context loading:

| Backend | Mechanism | Delivery | Behavior |
|---------|-----------|----------|----------|
| Claude | `--append-system-prompt-file` | CLI flag | Appends to system prompt |
| Codex | `model_instructions_file` | `-c model_instructions_file="/path"` | Replaces AGENTS.md* |
| Gemini | `GEMINI_SYSTEM_MD` | Env var with path | Replaces system prompt |

*Our context already includes root *.md files (AGENTS.md, GEMINI.md, etc.) via `gather_lfdocs`,
so we're not losing that content—we're just loading it through our file instead of the CLI's
native discovery.

## Implementation

### 1. Update `build_codex_command()` in `launcher.py`

Add `context_file` parameter:

```python
def build_codex_command(
    auto: bool,
    stream: bool,
    skip_permissions: bool,
    yolo: bool = False,
    model_variant: str | None = None,
    sandbox_root: Path | None = None,
    workdir: Path | None = None,
    images: list[Path] | None = None,
    context_file: Path | None = None,  # NEW
) -> list[str]:
    cmd = ["codex", "exec"]

    if context_file:
        cmd.extend(["-c", f'model_instructions_file="{context_file}"'])

    # ... rest unchanged
```

Same for `build_codex_interactive_command()`.

### 2. Update Gemini env handling

Gemini uses `GEMINI_SYSTEM_MD` env var to load a custom system prompt file.
Simpler than `--include-directories` (no extra dirs, no workspace pollution).

**Update `get_model_env()` in `logging.py`:**
```python
def get_model_env(
    strip_api_keys: bool = True,
    gemini_context_file: Path | None = None,
) -> dict[str, str]:
    env = os.environ.copy()
    if strip_api_keys:
        env.pop("ANTHROPIC_API_KEY", None)
        env.pop("OPENAI_API_KEY", None)
    if gemini_context_file:
        env["GEMINI_SYSTEM_MD"] = str(gemini_context_file)
    return env
```

No changes needed to `build_gemini_command()` itself—env is passed to subprocess.

### 3. Update `execution.py` (Python)

Remove the `supports_context_file` check and pass context to all backends.

### 4. Update `build_codex_command()` in `agent.rs` (Rust)

```rust
pub fn build_codex_command(config: &LaunchConfig) -> Vec<String> {
    let mut cmd = vec!["codex".to_string(), "exec".to_string()];

    // Load context via model_instructions_file
    if let Some(ref context_file) = config.context_file {
        cmd.push("-c".to_string());
        cmd.push(format!("model_instructions_file=\"{}\"", context_file.display()));
    }

    // ... rest unchanged
}
```

### 5. Update Gemini env in `launch_agent()` (Rust)

For Gemini, set `GEMINI_SYSTEM_MD` in the subprocess environment:

```rust
// In launch_agent() or similar
if model == "gemini" {
    if let Some(ref context_file) = config.context_file {
        cmd_env.insert("GEMINI_SYSTEM_MD".to_string(), context_file.to_string_lossy().to_string());
    }
}
```

### 6. Update `step.rs` (Rust)

Remove the `supports_context_file` check:

```rust
// Before
let supports_context_file = backend == "claude";

// After
// All backends support context files via their native mechanisms
// Always pass context_file to LaunchConfig
```

## File Changes

### Python

| File | Change |
|------|--------|
| `logging.py` | Extend `get_model_env()` to accept `gemini_context_file` |
| `launcher.py` | Add `context_file` to Codex builders |
| `execution.py` | Remove `supports_context_file` check; pass context to all backends |

### Rust

| File | Change |
|------|--------|
| `loopflow-engine/src/agent.rs` | Add `model_instructions_file` to `build_codex_command()` |
| `loopflow-engine/src/agent.rs` | Add `GEMINI_SYSTEM_MD` env handling for Gemini launches |
| `lf/src/commands/step.rs` | Remove `supports_context_file` check; always use context file |

## Testing

Manual verification for each backend:

```bash
# Claude - verify --append-system-prompt-file used
lf <step> -b claude --dry-run  # or inspect command

# Codex - verify -c model_instructions_file used
lf <step> -b codex --dry-run

# Gemini - verify GEMINI_SYSTEM_MD env var set
lf <step> -b gemini --dry-run
```

For each:
1. Context loads without appearing in input history
2. Task prompt is the primary user message
3. Clear error if context file fails to write

## Decisions

1. **Codex paths**: Use absolute paths for `model_instructions_file`
2. **Gemini**: Use `GEMINI_SYSTEM_MD` env var (simpler than `--include-directories`)
3. **Scope**: Both interactive and auto mode use context files
4. **Errors**: If context file can't be written or CLI rejects it, fail with clear error (no fallback)
5. **AGENTS.md/GEMINI.md content**: Already included via `gather_lfdocs` (root *.md files)

# Gemini Provider Support

Add Gemini CLI as a third model provider alongside Claude Code and Codex.

## What to build

A `GeminiRunner` class and supporting command builders that let users run `lf review -m gemini` or configure `agent_model: gemini:2.5-pro` in config.

## Data structures

No new data structures needed. The existing `Runner` ABC and `LaunchResult` dataclass work unchanged.

## Key functions

All in `src/loopflow/launcher.py`:

```python
class GeminiRunner(Runner):
    """Google Gemini CLI runner."""

    def launch(
        self,
        prompt: str,
        auto: bool = False,
        stream: bool = False,
        skip_permissions: bool = False,
        session_id: str | None = None,
        cwd: Optional[Path] = None,
    ) -> LaunchResult:
        """Launch using build_gemini_command + prompt as positional arg."""
        ...

    def is_available(self) -> bool:
        """Check for `gemini --version`."""
        ...


def build_gemini_command(
    auto: bool,
    stream: bool,
    skip_permissions: bool,
    model_variant: str | None = None,
    sandbox_root: Path | None = None,
    workdir: Path | None = None,
) -> list[str]:
    """Build Gemini CLI command.

    auto mode: uses positional prompt (no -p flag needed)
    stream mode: --output-format stream-json
    skip_permissions: --yolo or --approval-mode yolo
    model_variant: -m <variant>
    sandbox_root: not used (Gemini has different sandbox model)
    workdir: run from this directory
    """
    ...


def build_gemini_interactive_command(
    skip_permissions: bool,
    model_variant: str | None = None,
    sandbox_root: Path | None = None,
    workdir: Path | None = None,
) -> list[str]:
    """Build Gemini CLI command for interactive mode.

    Uses -i/--prompt-interactive to accept prompt then continue interactively.
    """
    ...


def normalize_gemini_event(event: dict) -> list[dict]:
    """Normalize Gemini stream-json events to common schema."""
    ...
```

Update `get_runner()`:

```python
def get_runner(model: str) -> Runner:
    runners = {
        "claude": ClaudeRunner,
        "codex": CodexRunner,
        "gemini": GeminiRunner,
    }
    ...
```

Update `build_model_command()` and `build_model_interactive_command()`:

```python
def build_model_command(...) -> list[str]:
    if model == "claude":
        return build_claude_command(...)
    if model == "gemini":
        return build_gemini_command(...)
    return build_codex_command(...)
```

## Gemini CLI flags mapping

| Loopflow concept | Gemini CLI flag |
|------------------|-----------------|
| auto mode | Positional prompt (no flag needed) |
| stream output | `--output-format stream-json` |
| skip permissions | `--yolo` or `--approval-mode yolo` |
| model variant | `-m <model>` (e.g., `gemini-2.5-pro`) |
| interactive + prompt | `-i <prompt>` (prompt-interactive) |
| sandbox | `-s` (optional, different model than Codex) |

## stream-json event normalization

Gemini's `stream-json` output needs mapping to the common schema used by `_format_normalized_event()`. The normalizer should handle:

- Tool use events → `{"type": "tool_use", "tool": ..., "input": ...}`
- Text output → `{"type": "text", "content": ...}`
- Completion → `{"type": "result", "status": "success"|"error"}`

Exact event schema TBD during implementation - inspect actual output with `gemini "test" --output-format stream-json`.

## Constraints

- **CLI availability**: Gemini CLI requires Node.js 20+. The `is_available()` check must verify `gemini --version` works.
- **Prompt passing**: Gemini uses positional args for prompts in non-interactive mode, not stdin. This matches how Claude and Codex work in loopflow.
- **Interactive mode**: Use `-i` flag which accepts the initial prompt then continues interactively.
- **Sandbox model**: Gemini's `-s` sandbox flag works differently from Codex's workspace sandboxing. For now, don't pass sandbox flags - let users configure via Gemini's own settings.

## Files to modify

1. `src/loopflow/launcher.py` - Add GeminiRunner, build functions, normalizer
2. `src/loopflow/cli/meta.py` - Add gemini to `doctor` and `install` commands
3. `README.md` - Update to mention Gemini support
4. `tests/test_backend.py` - Add test for gemini runner

## Done when

```bash
# Verify gemini runner exists and is selectable
uv run python -c "from loopflow.launcher import get_runner; r = get_runner('gemini'); print(type(r).__name__)"
# Output: GeminiRunner

# Verify command building
uv run python -c "
from loopflow.launcher import build_gemini_command
cmd = build_gemini_command(auto=True, stream=True, skip_permissions=True, model_variant='2.5-pro')
print(' '.join(cmd))
"
# Output should include: gemini -m 2.5-pro --output-format stream-json --yolo

# If gemini CLI installed, verify is_available
uv run python -c "from loopflow.launcher import get_runner; print(get_runner('gemini').is_available())"
# Output: True (if installed) or False

# Run tests
uv run pytest tests/test_backend.py -v
```

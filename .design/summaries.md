# Summaries Reliability

Make codebase summaries work as designed: visible in token breakdown, errors surfaced, dependencies correct.

## What to build

Fix three issues preventing summaries from working:
1. Missing `google-genai` dependency for gemini model
2. Silent failures in background refresh (stderr → DEVNULL)
3. Token breakdown ignores summaries entirely

## Current state

Config:
```yaml
summaries:
  - path: src/loopflow
    tokens: 16000
    model: gemini
```

Running `lf implement` shows no "summaries" section in token breakdown. Background refresh fails silently because gemini support isn't installed.

## Data structures

No new types. Existing `PromptComponents.summaries` already carries the data:

```python
@dataclass
class PromptComponents:
    ...
    summaries: list[tuple[Path, str]] | None = None
```

## Key functions

```python
# tokens.py - add summaries parameter
def analyze_prompt_tokens(
    ...
    summaries: list[tuple[Path, str]] | None = None,
) -> TokenTree:
    """Add summaries to token breakdown."""

# tokens.py - update to pass summaries
def analyze_components(components) -> TokenTree:
    return analyze_prompt_tokens(
        ...
        summaries=components.summaries,
    )
```

```python
# context.py - log errors instead of silencing
def _trigger_background_refresh(repo_root: Path) -> None:
    log_path = repo_root / ".lf" / "summaries" / "refresh.log"
    with open(log_path, "w") as log:
        process = subprocess.Popen(
            [sys.executable, "-m", "loopflow.lfops", "summarize", "--all"],
            cwd=repo_root,
            stdout=log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
```

## Changes

### 1. pyproject.toml - add google dependency

```toml
"pydantic-ai-slim[anthropic,google]>=1.0.0",
```

### 2. tokens.py - include summaries in breakdown

Add `summaries` parameter to `analyze_prompt_tokens`. Format similar to docs:

```python
if summaries:
    for summary_path, content in summaries:
        tokens = count_tokens(content)
        tree.add("summaries", str(summary_path), tokens)
```

Update `analyze_components` to pass `summaries=components.summaries`.

### 3. context.py - log background refresh output

Change `subprocess.DEVNULL` to a log file at `.lf/summaries/refresh.log`. This preserves errors for debugging without cluttering terminal.

### 4. gather_summaries - surface missing status

When summaries are configured but missing, `gather_summaries` currently returns empty list and triggers background refresh. Add a way to signal this:

```python
def gather_summaries(repo_root: Path, config) -> tuple[list[tuple[Path, str]], bool]:
    """Returns (summaries, any_missing)."""
```

Then in token breakdown, show placeholder:
```
summaries       (generating...)
  src/loopflow  pending
```

## Constraints

- Background refresh must remain non-blocking (first run shouldn't wait)
- Gemini is configured default; must work out of box after fix
- Error log location must be consistent with existing `.lf/summaries/` structure

## Done when

```bash
# 1. Dependency works
uv sync
uv run lfops summarize -a
# Should complete without google-genai error

# 2. Token breakdown shows summaries
uv run lf implement -c
# Output includes "summaries" section with token count

# 3. Errors logged
cat .lf/summaries/refresh.log
# Shows generation output, not empty
```

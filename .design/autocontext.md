# Autocontext: LLM-Generated Codebase Summaries

## What to build

`lfops summarize` generates token-budgeted codebase summaries via LLM, with optional background refresh to keep them eventually consistent with main.

## User quotes

> "lfops summarize command that takes a token-size limit and then communicates to an llm (gemini by default?) to produce a summary"

> "deeply in the style of Loopflow, prioritizing APIs and data structures, direct quotes"

> "let's add a way of keeping summarize(path, token_size) eventually consistent (i.e. some sort of background agent that checks diff against main)"

> "turn it on at root with an appropriate context limit to be able to include in all llm messages for the loopflow repo"

## Data structures

```python
# src/loopflow/summarize.py

@dataclass
class Summary:
    """A generated codebase summary."""
    path: Path              # What was summarized (file or directory)
    content: str            # The summary text
    token_budget: int       # Requested token limit
    source_hash: str        # Hash of source content (for staleness check)
    created_at: datetime
    model: str              # Model used (e.g., "gemini:2.5-pro")

def load_summary(path: Path, repo_root: Path) -> Summary | None:
    """Load cached summary from .lf/summaries/"""
    ...

def save_summary(summary: Summary, repo_root: Path) -> None:
    """Save summary to .lf/summaries/"""
    ...

def is_stale(summary: Summary, repo_root: Path) -> bool:
    """Check if source content changed since summary was generated."""
    ...
```

```python
# Config additions in config.py

class SummaryConfig(BaseModel):
    """Per-path summary configuration."""
    path: str              # Path to summarize (relative to repo root)
    tokens: int            # Token budget for this summary
    model: str = "gemini"  # Model to use

class Config(BaseModel):
    # ... existing fields ...
    summaries: list[SummaryConfig] = []  # Summaries to generate/include
```

```yaml
# .lf/config.yaml example
summaries:
  - path: .           # Root summary for whole repo
    tokens: 4000
  - path: src/loopflow/lfd
    tokens: 2000
```

## Key functions

```python
# src/loopflow/summarize.py

def gather_source_content(path: Path, repo_root: Path, exclude: list[str]) -> str:
    """Collect all file contents under path for summarization."""
    ...

def hash_content(content: str) -> str:
    """Hash content for staleness detection."""
    ...

def generate_summary(
    path: Path,
    repo_root: Path,
    token_budget: int,
    model: str = "gemini",
) -> Summary:
    """Generate summary via LLM, respecting token budget."""
    ...

def refresh_if_stale(summary_config: SummaryConfig, repo_root: Path) -> Summary:
    """Load cached summary or regenerate if stale."""
    ...
```

```python
# src/loopflow/context.py additions

def gather_summaries(repo_root: Path, config: Config) -> list[tuple[Path, str]]:
    """Load all configured summaries for context inclusion."""
    ...
```

```python
# CLI: src/loopflow/lfops.py

@app.command()
def summarize(
    path: str = typer.Argument(".", help="Path to summarize"),
    tokens: int = typer.Option(4000, "-t", "--tokens", help="Token budget"),
    model: str = typer.Option("gemini", "-m", "--model", help="Model to use"),
    force: bool = typer.Option(False, "-f", "--force", help="Regenerate even if cached"),
) -> None:
    """Generate a codebase summary."""
    ...
```

## File layout

```
.lf/
  config.yaml           # summaries: config
  summaries/
    root.md             # Summary for path="."
    src-loopflow-lfd.md # Summary for path="src/loopflow/lfd"
    _metadata.json      # Hash, timestamp, model for each summary
```

## Summary prompt

Lives at `src/loopflow/prompts/SUMMARIZE.md` (builtin, overridable via `.lf/SUMMARIZE.md`):

```markdown
Summarize this codebase for LLM context. Target: {token_budget} tokens.

Prioritize:
1. **Data structures** - Core types with field annotations
2. **Public APIs** - Function signatures with one-line descriptions
3. **Key patterns** - How the codebase is organized
4. **Direct quotes** - Preserve exact names, paths, commands

Format as dense markdown. No fluff. Code blocks for types/signatures.
Omit implementation details unless they're critical to understanding.

<source>
{content}
</source>
```

## Context integration

Summaries appear in `<lf:summaries>` section after docs, before files:

```xml
<lf:summaries>
<lf:summary path=".">
# loopflow

Run LLM coding agents from reusable prompt files...
</lf:summary>
</lf:summaries>
```

In `gather_prompt_components()`:
1. Load config summaries
2. For each, call `refresh_if_stale()`
3. Add to new `summaries` field in `PromptComponents`

## Background refresh

For eventually consistent summaries, use existing lfd agent infrastructure:

```markdown
# ~/.lf/agents/summarize-loopflow.md
---
repo: /Users/jack/src/loopflow
pipeline: summarize
trigger:
  kind: main-changed
---
```

Where `summarize` pipeline is:

```yaml
# .lf/config.yaml
pipelines:
  summarize:
    tasks: [refresh-summaries]
```

And `.claude/commands/refresh-summaries.md`:

```markdown
Run `lfops summarize` for each configured summary path.
Commit changes to .lf/summaries/ if any files updated.
```

Alternatively, simpler: `lfops summarize --all` regenerates all configured summaries.

## UI changes

### Maestro PromptLauncher

Add "Summaries" toggle in context options (alongside Docs, Files, Diff, Clipboard):

```swift
// PromptLauncher.swift - context toggles
Toggle("Summaries", isOn: $includeSummaries)
```

Token estimator includes summary tokens when enabled.

### Config display

Show configured summaries in Maestro's config panel (read-only, edit via config.yaml).

## Constraints

- **Token budget is approximate** — LLM output varies. Accept ±20% variance.
- **Gemini default** — Use Gemini for cost efficiency. Claude/Codex as options.
- **Source hash uses git** — `git ls-files -s | sha256sum` for directory, file content hash for single file.
- **No incremental updates** — Regenerate full summary when stale. Simplicity over efficiency.
- **Summaries are committed** — They live in `.lf/summaries/`, versioned with the repo.

## Done when

```bash
# Generate summary
lfops summarize . --tokens 4000
# => Creates .lf/summaries/root.md

# Check it's included in context
lf review -c | grep "lf:summary"
# => Shows <lf:summary path="."> section

# Config-based summaries
cat .lf/config.yaml
# summaries:
#   - path: .
#     tokens: 4000

lf implement -c | grep "lf:summary"
# => Summary included automatically

# Staleness check
echo "# change" >> README.md
lfops summarize .
# => "Summary stale, regenerating..."
```

Maestro verification:
1. Open loopflow repo in Maestro
2. Enable "Summaries" toggle
3. Token count increases by ~4000
4. Run task, verify summary appears in prompt

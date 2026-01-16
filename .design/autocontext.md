# Autocontext: LLM-Generated Codebase Summaries

## What was built

`lfops summarize` generates token-budgeted codebase summaries via LLM. Summaries are cached in `.lf/summaries/` with staleness detection based on git file hashes. Configured summaries are automatically included in prompt context.

## Implementation

### Data structures

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
    model: str              # Model used (e.g., "gemini")

@dataclass
class SummaryMetadata:
    """Metadata for a single summary (stored in _metadata.json)."""
    source_hash: str
    token_budget: int
    created_at: str
    model: str
```

```python
# src/loopflow/config.py

class SummaryConfig(BaseModel):
    """Per-path summary configuration."""
    path: str              # Path to summarize (relative to repo root)
    tokens: int            # Token budget for this summary
    model: str = "gemini"  # Model to use
```

### Config format

```yaml
# .lf/config.yaml
summaries:
  - path: .           # Root summary for whole repo
    tokens: 4000
  - path: src/loopflow/lfd
    tokens: 2000
```

### Key functions

```python
# src/loopflow/summarize.py

def load_summary(path: Path, repo_root: Path) -> Summary | None:
    """Load cached summary from .lf/summaries/."""

def save_summary(summary: Summary, repo_root: Path) -> None:
    """Save summary to .lf/summaries/."""

def is_stale(summary: Summary, repo_root: Path) -> bool:
    """Check if source content changed since summary was generated."""

def gather_source_content(path: Path, repo_root: Path, exclude: list[str] | None) -> str:
    """Collect file contents at merge-base for summarization."""

def generate_summary(path, repo_root, token_budget, model, exclude) -> Summary:
    """Generate summary via LLM. Returns raw content if it fits in budget."""

def refresh_if_stale(path, repo_root, token_budget, model, exclude, force) -> tuple[Summary, bool]:
    """Load cached summary or regenerate if stale. Returns (summary, was_regenerated)."""
```

```python
# src/loopflow/context.py

def gather_summaries(repo_root: Path, config) -> list[tuple[Path, str]]:
    """Load all configured summaries for context inclusion.

    Triggers background refresh if any summaries are stale or missing."""
```

### CLI

```bash
lfops summarize . --tokens 4000           # Generate summary for repo root
lfops summarize src/loopflow -t 2000      # Generate summary for subdirectory
lfops summarize . -f                      # Force regenerate
lfops summarize --all                     # Regenerate all configured summaries
```

### File layout

```
.lf/
  config.yaml           # summaries: config
  summaries/
    root.md             # Summary for path="."
    src-loopflow-lfd.md # Summary for path="src/loopflow/lfd"
    _metadata.json      # Hash, timestamp, model for each summary
```

### Prompt template

Lives at `src/loopflow/builtins/summarize.txt`, overridable via `.lf/SUMMARIZE.md`:

```
Summarize this codebase for LLM context. Target: {token_budget} tokens.

Prioritize:
1. **Data structures** - Core types with field annotations
2. **Public APIs** - Function signatures with one-line descriptions
3. **Key patterns** - How the codebase is organized
4. **Direct quotes** - Preserve exact names, paths, commands

Format as dense markdown. No fluff. Code blocks for types/signatures.
Omit implementation details unless they're critical to understanding.
```

### Context integration

Summaries appear in prompt context after docs, before files:

```xml
<lf:summaries>
<lf:summary path=".">
# loopflow

Run LLM coding agents from reusable prompt files...
</lf:summary>
</lf:summaries>
```

CLI flags: `--summaries/--no-summaries` to override config.

### Staleness detection

Source hash computed via `git ls-tree` at the merge-base with main. This means summaries only refresh when main advances, not on local branch modifications (which are already visible via diff_files). Background refresh is triggered automatically when stale summaries are detected during context gathering.

### Swift/Maestro

- `SummaryConfig` struct added to `LoopflowConfig.swift`
- `hasSummaries` computed property for UI
- `ConfigLoader.swift` updated to parse summaries (full YAML parsing not implemented - summaries field passed as nil from loader)
- `PromptLauncher.swift` adds "Summaries" toggle (shown when config has summaries)
- `TokenEstimator.swift` passes `--no-summaries` flag when disabled
- `AppState.swift` tracks `includeSummaries` state

## Implementation notes

- **Token budget bypass**: If source content fits within the token budget, it's returned directly without LLM summarization (model set to "raw")
- **Merge-base aware**: Summaries are generated from content at the merge-base with main, so they represent the base codebase state, not local branch changes
- **Background refresh**: Stale or missing summaries trigger a background `lfops summarize --all` process, using a lock file to prevent concurrent refreshes

## Usage

```bash
# Generate summary
lfops summarize . --tokens 4000

# Check it's included in context
lf review -c | grep "lf:summary"

# Configure in .lf/config.yaml, then summaries auto-include
lf implement -c | grep "lf:summary"
```

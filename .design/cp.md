# cp: Copy context to clipboard

## What to build

Add `lf cp` subcommand that gathers file context (like `lf run -c` but with no task) and copies it to clipboard.

## User intent

> "add a cp subcommand that accepts include/exclude/configured paths and builds a full context similar to how we do for lf prompts, but with no task, and just copy the context into the clipboard rather than forwarding to an llm coding agent"

## Data structures

Reuses existing `PromptComponents` from `context.py`. No new types needed.

```python
# context.py - already exists
@dataclass
class PromptComponents:
    run_mode: str | None
    docs: list[tuple[Path, str]]
    diff: str | None
    task: tuple[str, str] | None  # Will be None for cp
    context_files: list[tuple[Path, str]]
    repo_root: Path
    clipboard: str | None = None
```

## Key functions

```python
# cli/run.py - new command

def cp(
    context: list[str] = typer.Option(None, "-x", "--context"),
    exclude: list[str] = typer.Option(None, "-e", "--exclude"),
    paste: bool = typer.Option(False, "-v", "--paste"),
    no_docs: bool = typer.Option(False, "--no-docs"),
    no_diff: bool = typer.Option(False, "--no-diff"),
) -> None:
    """Copy file context to clipboard without running a task."""
    ...
```

**Behavior:**
1. Gather components via `gather_prompt_components()` with `task=None`
2. Apply exclude patterns from CLI + config
3. Format with `format_prompt()` (or a simpler variant)
4. Copy to clipboard via `pbcopy`
5. Show token breakdown via `analyze_components()`

## Implementation

### Changes to `cli/__init__.py`

Register `cp` as top-level command:

```python
app.command()(run_module.cp)

# Add to known_commands
known_commands = {
    "run",
    "pipeline",
    "inline",
    "cp",  # new
    "ops",
    "agent",
    "--help",
    "-h",
}
```

### New function in `cli/run.py`

```python
def cp(
    context: list[str] = typer.Option(
        None, "-x", "--context", help="Files to include"
    ),
    exclude: list[str] = typer.Option(
        None, "-e", "--exclude", help="Patterns to exclude"
    ),
    paste: bool = typer.Option(
        False, "-v", "--paste", help="Include clipboard content"
    ),
    no_docs: bool = typer.Option(
        False, "--no-docs", help="Exclude repo documentation (.md files)"
    ),
    no_diff: bool = typer.Option(
        False, "--no-diff", help="Exclude branch diff"
    ),
):
    """Copy file context to clipboard."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    config = load_config(repo_root)

    # Merge CLI context with config context
    all_context = list(context or [])
    if config.context:
        all_context.extend(config.context)

    # Merge exclude patterns
    exclude_patterns = list(exclude or [])
    if config.exclude:
        exclude_patterns.extend(config.exclude)

    components = gather_prompt_components(
        repo_root,
        task=None,  # No task
        context=all_context or None,
        exclude=exclude_patterns or None,
        paste=paste,
        run_mode=None,  # No run mode header
    )

    # Optionally strip docs/diff
    if no_docs:
        components.docs = []
    if no_diff:
        components.diff = None

    prompt = format_prompt(components)
    _copy_to_clipboard(prompt)

    tree = analyze_components(components)
    typer.echo(tree.format())
    typer.echo("\nCopied to clipboard.")
```

## Constraints

- Must work without a task file (task=None)
- Must respect config.context and config.exclude
- Must show token breakdown before copying (matches existing `-c` behavior)
- No new dependencies

## Done when

```bash
# Basic usage
lf cp -x src/loopflow/cli/
# Shows token breakdown, copies to clipboard

# With excludes
lf cp -x src/ -e "*.pyc" -e "__pycache__/**"
# Excludes matched patterns

# Minimal context (just files, no docs/diff)
lf cp -x src/context.py --no-docs --no-diff
# Only includes the specified file

# Verify clipboard
pbpaste | head -20
# Shows formatted context
```

# diffload: Two context sources from branch changes

## What to build

Add `diff_files` as a new context source (files touched by the branch). Change `diff` default to off. Both are independent toggles like `docs` and `paste`.

## Problem

"It seems like the LLMs do not realize they have the whole diff in their context."

A diff shows changes but lacks surrounding context. Loading the actual files touched by the branch gives the LLM the full picture—it can see the whole function, the class hierarchy, the imports.

## The two options

| Option | What it loads | Default | CLI flags |
|--------|--------------|---------|-----------|
| `diff_files` | Full content of files touched by branch | ON | `--diff-files/--no-diff-files` |
| `diff` | Raw `git diff main...HEAD` output | OFF | `--diff/--no-diff` |

Both can be on, both can be off, or either one. They're independent context sources, same as `docs`, `paste`, `clipboard`.

## Data structures

Add to `PromptComponents`:

```python
@dataclass
class PromptComponents:
    run_mode: str | None
    docs: list[tuple[Path, str]]
    diff: str | None
    diff_files: list[tuple[Path, str]]  # NEW
    task: tuple[str, str] | None
    context_files: list[tuple[Path, str]]
    # ...
```

Add to `Config`:

```python
class Config(BaseModel):
    # ...
    diff: bool = False        # CHANGED: was True
    diff_files: bool = True   # NEW
```

## Key functions

```python
def gather_diff_files(repo_root: Path, exclude: list[str] | None = None) -> list[str]:
    """Return file paths touched by this branch vs main.

    Uses git diff --name-only main...HEAD.
    Filters out deleted files (can't load those).
    Respects exclude patterns.
    """
```

## Changes by file

### `src/loopflow/config.py`

```python
class Config(BaseModel):
    # ...
    diff: bool = False        # change default
    diff_files: bool = True   # add new field
```

### `src/loopflow/context.py`

Add `gather_diff_files()`:

```python
def gather_diff_files(repo_root: Path, exclude: list[str] | None = None) -> list[str]:
    result = subprocess.run(
        ["git", "diff", "--name-only", "main...HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return []

    paths = []
    for line in result.stdout.strip().split("\n"):
        if not line:
            continue
        path = repo_root / line
        if path.exists():  # filter deleted files
            paths.append(line)
    return paths
```

Update `PromptComponents`:

```python
@dataclass
class PromptComponents:
    # ...
    diff_files: list[tuple[Path, str]] = field(default_factory=list)
```

Update `gather_prompt_components()` to call `gather_diff_files()` and pass to `gather_files()`.

Update `format_prompt()` to include diff_files in `<lf:files>` section (they merge with context_files).

### `src/loopflow/cli/run.py`

Add CLI flag to `run()`, `inline()`, `cp()`:

```python
diff_files: Optional[bool] = typer.Option(
    None, "--diff-files/--no-diff-files", help="Include files touched by branch"
),
```

Same pattern as existing `--diff/--no-diff`, `--docs/--no-docs`.

### `src/loopflow/tokens.py`

Update `analyze_prompt_tokens()` to show diff_files in token breakdown:

```python
def analyze_prompt_tokens(
    # ...
    diff_files: Optional[list[tuple[Path, str]]] = None,
) -> TokenTree:
    # ...
    if diff_files:
        for file_path, content in diff_files:
            tokens = count_tokens(content)
            tree.add("diff_files", file_path.name, tokens)
```

### `src/loopflow/maestro/markdown.py`

Add to `AgentFile`:

```python
@dataclass
class AgentFile:
    # ...
    diff: bool = False        # change default
    diff_files: bool = True   # add field
```

Update `parse_agent_file()` and `create_agent_file()`.

### `docs/config.md`

Document both options with their defaults and CLI flags.

## Constraints

- **Filter deleted files.** `git diff --name-only` includes deleted files—can't load those.
- **Respect exclude patterns.** If `exclude: ["*.test.ts"]` is set, don't load test files even if they're in the diff.
- **Merge with context_files.** diff_files should appear in the same `<lf:files>` section as explicit `-x` context, deduplicated.

## Done when

```bash
# On a feature branch with changes
lf : "list files in lf:files" -c 2>&1 | head -20
```

Should show:
- `diff_files` category in token breakdown (not `diff`)
- Changed files in `<lf:files>` section

With flags:

```bash
lf : "test" --no-diff-files --diff -c   # old behavior
lf : "test" --diff-files --no-diff -c   # new default
lf : "test" --diff-files --diff -c      # both
```

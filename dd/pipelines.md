# Pipelines

Chain tasks into named sequences that run non-interactively with autocommit after each step.

> "I basically first want somewhere to write an alias, so that implement, review, draft_commit was in some sense a single word that took one arg (the implement arg)"

## What to build

Named pipelines defined in `.lf/config.yaml` that chain existing tasks. Each task runs with `-p` (print mode), autocommits, then the next task starts.

```bash
lf ship design.md   # runs implement -> review -> draft_commit
```

## Data structures

```python
# In config.py or similar

@dataclass
class Pipeline:
    name: str
    tasks: list[str]  # ["implement", "review", "draft_commit"]

@dataclass
class Config:
    pipelines: dict[str, Pipeline]

def load_config(repo_root: Path) -> Config | None:
    """Load .lf/config.yaml. Returns None if not present."""
    ...
```

Config file format:

```yaml
# .lf/config.yaml
pipelines:
  ship:
    - implement
    - review
    - draft_commit
  quick:
    - implement
    - commit
```

## APIs

```python
# In pipeline.py

def run_pipeline(
    pipeline: Pipeline,
    repo_root: Path,
    arg: str | None = None,
    context: list[str] | None = None,
) -> int:
    """Run each task in sequence. Returns first non-zero exit code, or 0."""
    for i, task_name in enumerate(pipeline.tasks):
        # Only first task gets the arg
        task_arg = arg if i == 0 else None

        prompt = build_prompt(repo_root, task_name, arg=task_arg,
                              print_mode=True, context=context)
        exit_code, _ = launch_claude(prompt, print_mode=True, cwd=repo_root)

        if exit_code != 0:
            return exit_code

        # Autocommit after each task
        _autocommit(repo_root, task_name)

    _notify_done(pipeline.name)
    return 0

def _autocommit(repo_root: Path, task_name: str) -> None:
    """Commit any changes with message 'lf {task_name}'."""
    # git add -A && git commit -m "lf {task_name}" (if there are changes)
    ...

def _notify_done(pipeline_name: str) -> None:
    """Show macOS notification."""
    subprocess.run([
        "osascript", "-e",
        f'display notification "Pipeline complete" with title "lf {pipeline_name}"'
    ])
```

## CLI changes

```python
# In cli.py main()

def main():
    known_commands = {"run", "version", "install", "doctor", "land", "--help", "-h"}

    if len(sys.argv) > 1 and sys.argv[1] not in known_commands:
        # Check if it's a pipeline first
        repo_root = find_repo_root()
        config = load_config(repo_root) if repo_root else None

        if config and sys.argv[1] in config.pipelines:
            # Route to pipeline runner
            sys.argv.insert(1, "pipeline")
        else:
            # Existing behavior: route to run
            sys.argv.insert(1, "run")

    app()

@app.command()
def pipeline(
    name: str = typer.Argument(help="Pipeline name from config.yaml"),
    arg: str = typer.Argument(None, help="Input for first task"),
    context: list[str] = typer.Option(None, "-c", "--context", help="Context files"),
):
    """Run a named pipeline."""
    ...
```

## Constraints

- **Pipelines are always print mode** — no interactive pipeline runs
- **Name collision is an error** — if both `.lf/ship.lf` and `pipelines.ship` exist, fail with clear message
- **First task gets the arg** — subsequent tasks run without arg
- **Context applies to all tasks** — `-c` files included in every task's prompt
- **Autocommit after each task** — not at the end of the pipeline
- **Fail fast** — stop pipeline on first non-zero exit

## Done when

```bash
# Create config
cat > .lf/config.yaml << 'EOF'
pipelines:
  ship:
    - implement
    - review
    - draft_commit
EOF

# Run pipeline
lf ship dd/some-feature.md

# Should see:
# 1. implement runs with dd/some-feature.md as arg
# 2. autocommit with message "lf implement"
# 3. review runs (no arg)
# 4. autocommit with message "lf review"
# 5. draft_commit runs (no arg)
# 6. autocommit with message "lf draft_commit"
# 7. macOS notification pops up
```

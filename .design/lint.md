# Smart Lint: Fast Path with Agent Fallback

## What to build

Run lint checks directly first; only invoke the agent if they fail.

## Background

Currently `run_lint()` always invokes `lf lint -a`, spinning up an agent even when lint already passes. The rebase command shows the right pattern: try the fast path, escalate to agent only on failure.

## Config

Optional in `.lf/config.yaml`:

```yaml
lint_check: "ruff check src/ tests/ && ruff format --check src/ tests/"
```

If set, run this command to check if lint passes. If it exits 0, skip the agent.

If not set, fall back to auto-detection (ruff + src/tests dirs).

## Key functions

```python
def _check_lint(repo_root: Path, config: Config | None) -> bool | None:
    """Run lint check command. Returns True/False, or None if can't fast-check."""

def run_lint(repo_root: Path) -> bool:
    """Check lint first; invoke agent only if checks fail."""
```

### Flow

```
┌──────────────────┐
│ config.lint_check│  ← user-specified command (if set)
│    OR            │
│ auto-detect ruff │  ← fallback: ruff check + format --check
└────────┬─────────┘
         │
     passes? ────yes───→ return True (no agent)
         │
         no (or can't check)
         ↓
┌──────────────────┐
│ lf lint -a       │  ← agent uses lint.md
└────────┬─────────┘
         │
     passes? ────yes───→ return True
         │
         no
         ↓
     return False
```

## Changes

### `_helpers.py`

```python
def _check_lint(repo_root: Path, config: Config | None) -> bool | None:
    """Run lint check. Returns True if passes, False if fails, None if can't check."""
    # Try user-configured command first
    if config and config.lint_check:
        result = subprocess.run(
            config.lint_check,
            shell=True,
            cwd=repo_root,
            capture_output=True,
        )
        return result.returncode == 0

    # Fall back to auto-detect ruff
    if shutil.which("ruff") is None:
        return None

    targets = []
    if (repo_root / "src").is_dir():
        targets.append("src/")
    if (repo_root / "tests").is_dir():
        targets.append("tests/")
    if not targets:
        return None

    check = subprocess.run(["ruff", "check", *targets], cwd=repo_root, capture_output=True)
    if check.returncode != 0:
        return False

    fmt = subprocess.run(["ruff", "format", "--check", *targets], cwd=repo_root, capture_output=True)
    return fmt.returncode == 0


def run_lint(repo_root: Path) -> bool:
    """Check lint first; invoke agent only if checks fail."""
    config = load_config(repo_root)
    result = _check_lint(repo_root, config)

    if result is True:
        typer.echo("Lint passed")
        return True

    if result is False:
        typer.echo("Lint issues found, running fixer...")
    else:
        typer.echo("Running lint...")

    agent_result = subprocess.run(["lf", "lint", "-a"], cwd=repo_root)
    return agent_result.returncode == 0
```

### `config.py`

Add `lint_check` field to Config:

```python
@dataclass
class Config:
    # ... existing fields ...
    lint_check: str | None = None  # command to check if lint passes
```

### `commit.py`

Add `--lint/--no-lint` flag to match pr/land:

```python
def commit(
    # ... existing flags ...
    lint: bool = typer.Option(True, "--lint/--no-lint", help="Run lint before commit"),
) -> None:
    # after staging, before agent commit:
    if lint and not run_lint(repo_root):
        typer.echo("Lint failed, aborting commit", err=True)
        raise typer.Exit(1)
```

## Constraints

- If `lint_check` configured: use that command for fast path
- If not configured: auto-detect ruff + src/tests dirs
- If can't fast-check: go straight to agent
- Agent uses customizable `lint.md` step file

## Done when

```bash
# Fast path - lint passes, no agent:
$ lfops pr
Lint passed
Creating PR...

# Fast path - lint fails, agent fixes:
$ echo "import os" >> src/loopflow/__init__.py
$ lfops pr
Lint issues found, running fixer...
[agent removes unused import]
Creating PR...

# Custom lint_check in config:
# .lf/config.yaml: lint_check: "npm run lint"
$ lfops pr
Lint passed
Creating PR...
```

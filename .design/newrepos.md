# newrepos: Fix Init and Improve New Repo Experience

## What to build

Two things:
1. Fix `lf ops init` to actually work (currently broken - references missing directories)
2. Better first-run experience when loopflow is run in an uninitialized repo

## Part 1: Fix `lf ops init`

### Current Problems

**Missing commands directory** — The init code references paths that don't exist:

```python
# cli/meta.py:81
commands_src = bundled_dir / "commands"  # This path doesn't exist!

for prompt_name in ["review.md", "implement.md", ...]:
    src = commands_src / prompt_name  # FileNotFoundError
```

**Loopflow-specific prompts** — Current prompts reference this codebase specifically:

```markdown
# From implement.md
- **Use `uv run`** for all Python commands
- **Imports at top of file**, never inline
```

**Missing PROMPTS.md** — The workflow documentation isn't copied.

### Solution: Bundle templates in package

```
src/loopflow/
├── templates/              # NEW: bundled templates
│   ├── commands/           # Starter prompts for .claude/commands/
│   │   ├── design.md
│   │   ├── implement.md
│   │   ├── review.md
│   │   └── debug.md
│   ├── PROMPTS.md          # Workflow documentation
│   ├── STYLE.md            # Generic style guide
│   └── config.yaml         # Default config
├── prompts/                # Keep: internal prompts for lf itself
│   ├── COMMIT_MESSAGE.md
│   └── CHECKPOINT_MESSAGE.md
```

### What gets copied where

| Source | Destination |
|--------|-------------|
| `templates/commands/*.md` | `.claude/commands/` |
| `templates/PROMPTS.md` | `.lf/PROMPTS.md` |
| `templates/STYLE.md` | `.lf/STYLE.md` |
| `templates/config.yaml` | `.lf/config.yaml` |
| `prompts/COMMIT_MESSAGE.md` | `.lf/COMMIT_MESSAGE.md` |
| `prompts/CHECKPOINT_MESSAGE.md` | `.lf/CHECKPOINT_MESSAGE.md` |

### Starter prompt set

Minimal set (4 prompts) that forms a complete workflow:

- `design.md` — Plan before coding
- `implement.md` — Execute the plan
- `review.md` — Check the work
- `debug.md` — Fix errors

Power users who want polish, iterate, reduce, expand, etc. can add them manually or use `--all`.

### Generic prompts

Strip loopflow-specific references:

```markdown
# Before (loopflow-specific)
- **Use `uv run`** for all Python commands
- **Imports at top of file**, never inline

# After (generic)
Follow your project's existing patterns. Before writing new code,
find similar code nearby and match its style.
```

### Key functions

```python
# cli/meta.py

def _get_templates_dir() -> Path:
    """Return path to bundled templates directory."""
    return Path(__file__).parent.parent / "templates"

@app.command()
def init(
    prompts_only: bool = typer.Option(False, "--prompts", help="Only install prompts"),
    style_only: bool = typer.Option(False, "--style", help="Only install style guide"),
    all_prompts: bool = typer.Option(False, "--all", help="Install all prompts, not just starter set"),
):
    """Initialize a repository with loopflow prompts and config."""
    templates = _get_templates_dir()

    starter_prompts = ["design.md", "implement.md", "review.md", "debug.md"]
    prompts_to_copy = (
        list((templates / "commands").glob("*.md"))
        if all_prompts
        else [templates / "commands" / p for p in starter_prompts]
    )

    # Copy prompts to .claude/commands/
    commands_dir = repo_root / ".claude" / "commands"
    commands_dir.mkdir(parents=True, exist_ok=True)
    for src in prompts_to_copy:
        dst = commands_dir / src.name
        if not dst.exists():
            shutil.copy(src, dst)
            typer.echo(f"✓ Created .claude/commands/{src.name}")

    # Copy workflow docs to .lf/
    lf_dir = repo_root / ".lf"
    lf_dir.mkdir(exist_ok=True)

    for name in ["PROMPTS.md", "STYLE.md", "config.yaml"]:
        src = templates / name
        dst = lf_dir / name
        if not dst.exists():
            shutil.copy(src, dst)
            typer.echo(f"✓ Created .lf/{name}")
```

---

## Part 2: Better Error Messages

### Data structures

```python
@dataclass
class InitStatus:
    """What's configured in the current repo."""
    has_lf_dir: bool         # .lf/ exists
    has_config: bool         # .lf/config.yaml exists
    has_commands: bool       # .claude/commands/ has any .md files
    missing_deps: list[str]  # ["claude", "wt"] etc.

def check_init_status(repo_root: Path) -> InitStatus:
    """Check repo initialization without modifying anything."""
    lf_dir = repo_root / ".lf"
    commands_dir = repo_root / ".claude" / "commands"

    return InitStatus(
        has_lf_dir=lf_dir.exists(),
        has_config=(lf_dir / "config.yaml").exists(),
        has_commands=any(commands_dir.glob("*.md")) if commands_dir.exists() else False,
        missing_deps=_check_deps(),
    )
```

### Changes to cli/__init__.py

When task not found, check if repo is initialized:

```python
else:
    status = check_init_status(repo_root) if repo_root else None
    if status and not status.has_commands and not status.has_lf_dir:
        # Uninitialized repo - suggest init
        typer.echo(f"No task named '{name}' found.", err=True)
        typer.echo("", err=True)
        typer.echo("This repo hasn't been set up for loopflow yet.", err=True)
        typer.echo("Run: lf ops init", err=True)
    else:
        # Initialized but task missing - suggest creating it
        typer.echo(f"No task or pipeline named '{name}'", err=True)
        typer.echo(f"Create: .claude/commands/{name}.md", err=True)
    raise SystemExit(1)
```

### Changes to doctor command

Add repo status check:

```python
# Repo status
repo_root = find_worktree_root()
if repo_root:
    status = check_init_status(repo_root)
    if status.has_commands:
        typer.echo("✓ task files found")
    else:
        typer.echo("- no task files (run: lf ops init)")
else:
    typer.echo("- not in a git repo")
```

---

## Constraints

1. **No auto-creation** — Never create files without explicit `lf ops init`
2. **Works without init** — `lf : "fix typo"` should work in any git repo
3. **Don't break existing repos** — Only create files that don't exist
4. **Package must be self-contained** — Templates bundled, not read from source repo
5. **pyproject.toml** — Must include templates in package data

---

## Files to create/modify

**New files:**
- `src/loopflow/templates/commands/design.md` — Generic version
- `src/loopflow/templates/commands/implement.md` — Generic version
- `src/loopflow/templates/commands/review.md` — Generic version
- `src/loopflow/templates/commands/debug.md` — Generic version
- `src/loopflow/templates/PROMPTS.md` — Copy from repo root
- `src/loopflow/templates/STYLE.md` — Generic style guide
- `src/loopflow/templates/config.yaml` — Move from config_template.yaml
- `src/loopflow/init_check.py` — InitStatus dataclass and check_init_status()

**Modified files:**
- `src/loopflow/cli/meta.py` — Fix init paths, add --all flag
- `src/loopflow/cli/__init__.py` — Better error messages
- `pyproject.toml` — Include templates in package data

**Delete:**
- `src/loopflow/config_template.yaml` — Moved to templates/
- `src/loopflow/LOOPFLOW_STYLE.md` — Replaced by templates/STYLE.md

---

## Done when

```bash
# In a fresh git repo:
cd /tmp && rm -rf test-init && mkdir test-init && cd test-init && git init

# Run init
lf ops init

# Verify files created
test -f .claude/commands/design.md && echo "✓ design.md"
test -f .claude/commands/implement.md && echo "✓ implement.md"
test -f .claude/commands/review.md && echo "✓ review.md"
test -f .claude/commands/debug.md && echo "✓ debug.md"
test -f .lf/PROMPTS.md && echo "✓ PROMPTS.md"
test -f .lf/STYLE.md && echo "✓ STYLE.md"
test -f .lf/config.yaml && echo "✓ config.yaml"

# Verify prompts are generic
grep -q "uv run" .claude/commands/*.md && echo "FAIL: loopflow-specific" || echo "✓ generic"

# Test error message in uninitialized repo
cd /tmp && rm -rf test-uninit && mkdir test-uninit && cd test-uninit && git init
lf review 2>&1 | grep -q "lf ops init" && echo "✓ suggests init"

# Test inline prompts work without init
lf : "hello" --help 2>&1 | grep -q "inline" && echo "✓ inline works"

# Cleanup
rm -rf /tmp/test-init /tmp/test-uninit
```

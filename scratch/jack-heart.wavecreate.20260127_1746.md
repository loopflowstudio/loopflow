# Wave Creation with Worktree

Combine `lfd create` and `lfops wt create` so creating a wave also creates its worktree and switches you there.

## What to build

`lfd create <wave>` creates a wave record, creates a worktree for it, and switches to that worktree.

## Current state

```
lfd create swift-falcon     # just creates database record
lfops wt create swift-falcon  # separate: creates worktree, switches there
```

User then needs to manually coordinate: create wave, create worktree, configure wave with area/direction/flow.

## Proposed behavior

```bash
lfd create swift-falcon
# Output:
# Created wave: swift-falcon (abc123)
# Created worktree: ../loopflow.swift-falcon
# cd ../loopflow.swift-falcon
```

Wave name = worktree name. The worktree uses the existing branch schema from config.

## Data structures

No new types. Uses existing:

```python
# From lfd/wave.py
def create_wave(repo: Path, name: str | None = None) -> Wave: ...

# From lf/worktrees.py
@dataclass
class WorktreeResult:
    path: Path
    branch: str
    base_branch: str | None
```

## Key functions

```python
# lfd/cli.py - modify existing create command
@app.command()
def create(
    name: str = typer.Argument(None, help="Wave name (generated if omitted)"),
    no_worktree: bool = typer.Option(False, "--no-worktree", help="Skip worktree creation"),
):
    """Create a new wave with its worktree."""
    # 1. Create wave record
    wave = create_wave(repo=repo, name=name)

    # 2. Create worktree (unless --no-worktree)
    if not no_worktree:
        wt_result = create_with_schema(repo, wave.name, base=None, branch_config=config)
        # Write cd directive for shell integration
        write_directive(f"cd {wt_result.path}")
```

Import needed:
```python
from loopflow.lf.worktrees import create_with_schema
from loopflow.lfops.shell import write_directive
```

## Constraints

- Wave name must work as worktree name (no special chars)
- If wave exists, don't create duplicate worktree
- If worktree creation fails, still keep the wave (user can retry worktree manually)
- Shell integration (`write_directive`) only works if user ran `lfops shell install`

## Edge cases

1. **Wave exists**: Current behavior - prints "Wave already exists", returns. No worktree created.
2. **Worktree exists**: Check before creating. If worktree exists for this name, skip creation but still print cd.
3. **Auto-generated name**: `lfd create` with no name generates one. Worktree uses same generated name.

## Done when

```bash
# Fresh start
lfd rm swift-falcon --force 2>/dev/null
rm -rf ../loopflow.swift-falcon 2>/dev/null

# Create wave
lfd create swift-falcon

# Verify wave exists
lfd show swift-falcon  # should show wave details

# Verify worktree exists
ls ../loopflow.swift-falcon  # should exist

# Verify shell got cd directive (if shell integration active)
# Or: output shows "cd ../loopflow.swift-falcon"
```

## Open questions

1. Should there be a `--switch` flag that's default-on? Or always switch?
2. Should `lfd loop/watch/cron` also create worktree if wave doesn't exist? (Currently they create wave if --area provided, but no worktree)

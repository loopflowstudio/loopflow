# Wave Creation with Worktree

Make `lfd create` report the worktree it creates and switch the user there.

## What to build

`lfd create` already creates a worktree via `create_wave()`. The CLI just needs to:
1. Report the worktree path to the user
2. Switch to it via shell directive

## Current state

`wave.py:create_wave()` already creates a worktree:
```python
main_branch = f"{wave_name}.main"
worktree_path = create_worktree(repo, main_branch)
```

But `lfd create` CLI output doesn't mention it:
```
Created swift-falcon (abc123)
  Repo: /path/to/repo

Configure before running:
  lfd area swift-falcon src/
  ...
```

The HTTP API (`POST /waves`) returns `worktree` in the response, so Concerto already gets this info.

## Proposed behavior

```bash
lfd create swift-falcon
# Output:
# Created swift-falcon (abc123)
#   Worktree: ../loopflow.swift-falcon.main
#   Branch: swift-falcon.main
#
# cd ../loopflow.swift-falcon.main
```

Shell integration switches you there automatically.

## Key changes

### 1. CLI output (lfd/cli.py:249-288)

```python
@app.command()
def create(
    name: str = typer.Argument(None, help="Wave name (generated if omitted)"),
):
    wave = create_wave(repo=repo, name=name)

    typer.echo(f"{c['green']}Created{c['reset']} {c['bold']}{wave.name}{c['reset']} ({wave.short_id()})")

    if wave.worktree:
        typer.echo(f"  Worktree: {wave.worktree}")
        typer.echo(f"  Branch: {wave.branch}")

        # Switch to worktree via shell integration
        from loopflow.lfops.shell import write_directive
        if not write_directive(f"cd {wave.worktree}"):
            typer.echo(f"\ncd {wave.worktree}")
    else:
        typer.echo(f"  Repo: {repo}")
        typer.echo("  (worktree creation failed)")
```

### 2. Concerto (already works)

`WaveService.createWave()` calls `POST /waves` which returns `worktree` field.
Concerto can use `wave.worktreePath` to open Cursor/Warp there.

## Constraints

- Shell directive only works if `lfops shell install` was run
- If worktree creation failed, wave still exists (user sees warning)
- Branch naming: `{wave_name}.main` (existing convention)

## Done when

```bash
# Create wave and verify it switches you there
lfd create test-wave
pwd  # should be ../loopflow.test-wave.main (if shell integration active)

# Clean up
lfd rm test-wave --force
rm -rf ../loopflow.test-wave.main
```

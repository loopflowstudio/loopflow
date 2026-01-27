# Continuous Flow Execution

Run flows seamlessly: interactive steps launch the coding agent, auto steps follow when you Ctrl+D.

## What was built

When `lf <flow>` runs a flow containing interactive steps, those steps launch directly (full coding agent session). When the user exits (Ctrl+D), execution continues with remaining steps in auto mode.

```bash
lf design-and-ship  # design (interactive) → implement → reduce → polish (auto)
```

User experience:
1. Coding agent opens with `design` prompt
2. User collaborates on design, writes `scratch/<branch>.md`
3. User presses Ctrl+D to finish
4. Flow automatically continues: implement → reduce → polish
5. Notification when done

## Changes

### flows.py

Added `interactive` field to `Step` dataclass:

```python
@dataclass
class Step:
    name: str
    after: str | list[str] | None = None
    model: str | None = None
    direction: str | None = None
    interactive: bool | None = None  # Override frontmatter setting
```

### flow.py

Added `_is_step_interactive()` to check if a step should run interactively:

```python
def _is_step_interactive(step: Step, repo_root: Path, config: Config | None) -> bool:
    """Check if step should run interactively.

    Priority: flow step override > frontmatter > default (False)
    """
```

Added `_run_interactive_step()` to run steps interactively within a flow:

```python
def _run_interactive_step(...) -> int:
    """Run step interactively - direct coding agent, no collector.

    Unlike standalone interactive runs (which use os.execvp), this uses
    subprocess.run() so the flow can continue after the step completes.
    """
```

Modified `run_flow()` (renamed from `run_flow_def`) to check each step for interactive mode.

### Renames

- `run_flow_def` → `run_flow` (cleaner naming)

### New builtin flow

Added `design-and-ship` flow: `design → implement → reduce → polish`

## Verification

```bash
# Run the design-and-ship flow
lf design-and-ship

# Expected:
# 1. Coding agent opens with design prompt
# 2. User interacts, Ctrl+D to finish
# 3. Terminal shows "[2/4] implement" and continues auto
# 4. Notification when done
```

Manual verification:
- [ ] `lf design-and-ship` opens coding agent for design
- [ ] After Ctrl+D, implement/reduce/polish run automatically
- [ ] Exit code propagates (fail fast on any step failure)
- [ ] `lf ship` still works (no interactive step, all auto)

# Continuous Flow Execution

Run flows seamlessly: interactive steps launch Claude Code, auto steps follow when you Ctrl+D.

## What to build

When `lf <flow>` runs a flow containing interactive steps, those steps launch directly (full Claude Code session). When the user exits (Ctrl+D), execution continues with remaining steps in auto mode.

```bash
lf fullship  # design (interactive) → implement → reduce → polish (auto)
```

User experience:
1. Claude Code opens with `design` prompt
2. User collaborates on design, writes `scratch/<branch>.md`
3. User presses Ctrl+D to finish
4. Flow automatically continues: implement → reduce → polish
5. Notification when done

## Data structures

Step already has `interactive` in frontmatter:

```python
# Already exists in frontmatter.py
class StepConfig(BaseModel):
    interactive: bool | None = None
    # ...
```

Flow step can specify run mode:

```python
# flows.py - extend Step dataclass
@dataclass
class Step:
    name: str
    after: list[str] | None = None
    model: str | None = None
    direction: str | None = None
    interactive: bool | None = None  # New: override frontmatter
```

## Key functions

```python
# flow.py

def _is_step_interactive(step: Step, repo_root: Path, config: Config) -> bool:
    """Check if step should run interactively.

    Priority: flow override > frontmatter > default (False)
    """
    if step.interactive is not None:
        return step.interactive
    step_file = gather_step(repo_root, step.name, config)
    if step_file and step_file.config.interactive is not None:
        return step_file.config.interactive
    return False


def _run_interactive_step(
    params: _StepParams,
    repo_root: Path,
    step_num: int,
    total_steps: int,
    chrome: bool = False,
) -> int:
    """Run step interactively - direct Claude Code, no collector."""
    # Build prompt, launch Claude Code directly
    # Returns when user Ctrl+D
```

Change `run_flow_def` to:
1. Check if first step is interactive
2. If yes: run it interactively, then continue with remaining steps in auto mode
3. If no: run all steps in auto mode (current behavior)

## Constraints

**Interactive steps must be first.** An interactive step in the middle of a flow breaks the mental model. For MVP, only the first step can be interactive. Later: could support interactive steps anywhere with confirmation prompts.

**One interactive step per flow.** Multiple interactive steps would require complex state management. For MVP, first interactive step runs, rest are auto.

**User already on feature branch.** Interactive design assumes worktree exists. The flow command doesn't create worktrees for you.

## UI changes

None. This is purely execution behavior. The CLI already dispatches `lf <flow>` correctly.

## Done when

```bash
# Create a test flow with interactive first step
cat > .lf/flows/fullship.py << 'EOF'
from loopflow.lf.flows import Flow
def flow():
    return Flow("design", "implement", "reduce", "polish")
EOF

# Run it
lf fullship

# Expected:
# 1. Claude Code opens with design prompt
# 2. User interacts, Ctrl+D to finish
# 3. Terminal shows "[2/4] implement" and continues auto
# 4. Notification when done
```

Manual verification:
- [ ] `lf fullship` opens Claude Code for design
- [ ] After Ctrl+D, implement/reduce/polish run automatically
- [ ] Exit code propagates (fail fast on any step failure)
- [ ] `lf ship` still works (no interactive step, all auto)

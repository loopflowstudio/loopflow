# newrepos: Improve New Repository Experience

## What to build

Streamlined first-run experience that guides users from `pip install loopflow` to running their first task without requiring manual discovery of `lf ops init`.

## Context

Branch name `newrepos` suggests improving the new-repo onboarding flow. Current gaps identified:

1. Users must know to run `lf ops init` — no prompting
2. No dependency check before first task
3. Silent feature degradation when config missing
4. No clear guidance on `.claude/commands/` vs `.lf/` distinction

## Data structures

```python
@dataclass
class InitStatus:
    """What's been initialized in current repo."""
    has_config: bool
    has_prompts: bool
    has_style: bool
    missing_deps: list[str]  # e.g. ["claude", "worktrunk"]

def check_init_status(repo_root: Path) -> InitStatus:
    """Check what's initialized without modifying anything."""
    ...
```

## Key functions

```python
def prompt_init_if_needed(repo_root: Path) -> None:
    """Auto-prompt init on first task run if nothing configured."""
    ...

def suggest_missing_deps() -> list[str]:
    """Return list of missing but recommended dependencies."""
    ...

def run_with_init_check(task: str, repo_root: Path) -> None:
    """Wrap task execution with init status check."""
    ...
```

## Approach options

**Option A: Auto-prompt on first run**
- When `lf <task>` is run and no `.lf/` exists, prompt: "No loopflow config found. Run `lf ops init`? [Y/n]"
- Minimal change, preserves explicit control

**Option B: Auto-init with defaults**
- When no config exists, silently create minimal `.lf/config.yaml`
- Risky: creates files user didn't ask for

**Option C: Enhanced error messages only**
- Keep current behavior but improve error messages
- Add "Hint: run `lf ops init` to set up this repository"
- Lowest risk, minimal benefit

## Constraints

- Must work in auto/headless mode (no interactive prompts in `-a` mode)
- Cannot create files without user consent in interactive mode
- Must not break existing workflows where users skip init intentionally

## Done when

```bash
# In a fresh git repo with no .lf/ directory:
cd /tmp && mkdir test-repo && cd test-repo && git init

# Running a task shows helpful guidance:
lf review 2>&1 | grep -q "lf ops init"

# Doctor check works without init:
lf ops doctor  # exits 0 if deps installed, shows status
```

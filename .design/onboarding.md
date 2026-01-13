# Onboarding

Improve the first-run experience for new loopflow users.

## What to build

A guided setup flow that helps users go from `pip install loopflow` to running their first task, with clear feedback at each step.

## Current state

Today's onboarding is manual:
1. User installs loopflow
2. User runs `lf ops install` (if they know to)
3. User runs `lf ops init` to scaffold `.lf/`
4. User runs a task like `lf review`

Problems:
- No feedback when dependencies are missing
- No guidance on what to do next
- `lf ops doctor` exists but users don't know to run it

## Proposed approach

### Option A: First-run wizard

When user runs any `lf` command and setup is incomplete, prompt them through setup:

```
$ lf review

Welcome to loopflow! Let's get you set up.

Checking dependencies...
  ✓ Node.js
  ✗ Claude Code — installing...
  ✓ worktrunk
  ✓ Warp
  ✓ Cursor

Initialize this repo? [Y/n]
  ✓ Created .lf/config.yaml
  ✓ Created .claude/commands/

Ready! Running 'lf review'...
```

### Option B: Explicit init command

Keep current behavior but improve `lf ops init`:

```
$ lf ops init

Checking dependencies...
  ✓ Node.js
  ✗ Claude Code

Some dependencies are missing. Install them? [Y/n]
  Installing Claude Code...
  ✓ Claude Code installed

Creating .lf/ structure...
  ✓ .lf/config.yaml
  ✓ .claude/commands/review.md
  ...

Next steps:
  1. Run 'lf design' to start a new feature
  2. Or 'lf review' to review current changes
```

### Option C: Status-aware help

`lf --help` detects missing setup and shows relevant guidance:

```
$ lf --help

⚠️  Setup incomplete. Run 'lf ops init' to get started.

Usage: lf [OPTIONS] COMMAND [ARGS]...
...
```

## Data structures

```python
@dataclass
class SetupStatus:
    """Current setup state for a repo."""
    has_config: bool
    has_prompts: bool
    dependencies: dict[str, bool]  # name -> installed

    @property
    def is_complete(self) -> bool:
        return (
            self.has_config
            and self.has_prompts
            and all(self.dependencies.values())
        )

def check_setup(repo_root: Path) -> SetupStatus:
    """Check what's configured and what's missing."""
    ...
```

## APIs

```python
def check_setup(repo_root: Path | None) -> SetupStatus:
    """Check setup status for repo (or globally if None)."""
    ...

def run_setup(
    repo_root: Path,
    interactive: bool = True,
    install_deps: bool = True,
) -> SetupStatus:
    """Run setup flow, optionally interactive."""
    ...
```

## Constraints

- macOS only (per README)
- Should work without API keys (uses CLI subscriptions)
- Don't block experienced users who know what they're doing
- Keep it fast—don't check network on every command

## Done when

```bash
# Fresh repo, no prior setup
cd /tmp/test-repo && git init
lf ops init
# Shows dependency check, installs missing, creates .lf/

lf ops doctor
# All green

lf review
# Runs without additional prompts
```

## Open questions

See `.design/questions.md`

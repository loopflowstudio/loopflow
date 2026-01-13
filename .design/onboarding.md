# Onboarding

Improve `lf ops init` to be a unified first-run experience.

## What to build

Enhance `lf ops init` to check dependencies, offer to install missing ones, and scaffold the repo—all in one command. Users run one command to go from zero to ready.

## Decisions

Based on codebase exploration:

1. **Option B: Improved `lf ops init`** — The explicit approach. Don't intercept arbitrary commands (Option A adds complexity, can annoy experienced users). Don't just hint in help (Option C is too passive).

2. **Prompt before installing** — Ask "Install missing dependencies? [Y/n]" rather than auto-installing. Respects user control. Use `--yes` flag for CI/scripts.

3. **macOS only** — Fail gracefully with "macOS required" message on other platforms. Don't attempt partial support.

## Current state (from code)

- `init()` in `meta.py:63-150` scaffolds `.lf/` and `.claude/commands/`
- `install()` in `meta.py:160-264` installs deps via Homebrew/npm
- `doctor()` in `meta.py:266-327` checks what's installed
- These are separate commands with no integration

## Data structures

```python
@dataclass
class SetupStatus:
    """What's installed and what's missing."""
    node: bool
    claude: bool
    worktrunk: bool
    warp: bool      # IDE, optional per config
    cursor: bool    # IDE, optional per config
    codex: bool     # optional
    gemini: bool    # optional
    has_config: bool
    has_prompts: bool

    @property
    def ready(self) -> bool:
        """Can run tasks (required deps + repo setup)."""
        return self.node and self.claude and self.worktrunk and self.has_config

    @property
    def missing_required(self) -> list[str]:
        """Names of missing required dependencies."""
        missing = []
        if not self.node:
            missing.append("node")
        if not self.claude:
            missing.append("claude")
        if not self.worktrunk:
            missing.append("worktrunk")
        return missing
```

## Key functions

```python
def check_setup(repo_root: Path | None = None) -> SetupStatus:
    """Check all dependencies and repo state. Fast (no network)."""
    ...

def run_init(
    repo_root: Path,
    install_deps: bool = True,
    yes: bool = False,
) -> SetupStatus:
    """
    Unified init flow:
    1. Check dependencies
    2. Offer to install missing (if install_deps and not yes, prompt)
    3. Scaffold .lf/ and .claude/commands/
    4. Print next steps
    """
    ...
```

## Changes to meta.py

### Modify `init()` command

```python
@app.command()
def init(
    prompts: bool = typer.Option(False, "--prompts", help="Only install prompts"),
    style: bool = typer.Option(False, "--style", help="Only install style guide"),
    yes: bool = typer.Option(False, "--yes", "-y", help="Auto-confirm prompts"),
):
    """Initialize repo with loopflow. Checks deps, installs missing, scaffolds config."""
    repo_root = find_worktree_root()
    if not repo_root:
        print("Not in a git repository")
        raise typer.Exit(1)

    # If specific flags, use existing behavior
    if prompts or style:
        _install_subset(repo_root, prompts=prompts, style=style)
        return

    # Full init flow
    status = check_setup(repo_root)

    # Show current state
    _print_status(status)

    # Handle missing deps
    if status.missing_required:
        if yes or typer.confirm("Install missing dependencies?", default=True):
            _install_missing(status)
            status = check_setup(repo_root)  # recheck
        else:
            print("\nRun 'lf ops install' to install dependencies manually.")
            raise typer.Exit(1)

    # Scaffold repo
    _scaffold_repo(repo_root)

    # Success message
    print("\n✓ Ready! Try 'lf review' or 'lf design'")
```

### Add `check_setup()` function

Reuse existing check logic from `doctor()`:

```python
def check_setup(repo_root: Path | None = None) -> SetupStatus:
    """Check dependencies and repo state."""
    return SetupStatus(
        node=_check_available("npm"),
        claude=check_claude_available(),
        worktrunk=_check_available("wt"),
        warp=_check_available("warp"),
        cursor=_check_available("cursor"),
        codex=check_codex_available(),
        gemini=check_gemini_available(),
        has_config=repo_root and (repo_root / ".lf" / "config.yaml").exists(),
        has_prompts=repo_root and (repo_root / ".claude" / "commands").exists(),
    )

def _check_available(cmd: str) -> bool:
    """Check if command exists (no --version call, just which)."""
    return shutil.which(cmd) is not None
```

### Add `_print_status()` helper

```python
def _print_status(status: SetupStatus) -> None:
    """Print dependency check results."""
    print("Checking dependencies...")

    def icon(ok: bool) -> str:
        return "✓" if ok else "✗"

    print(f"  {icon(status.node)} Node.js")
    print(f"  {icon(status.claude)} Claude Code")
    print(f"  {icon(status.worktrunk)} worktrunk")

    if status.missing_required:
        print(f"\nMissing: {', '.join(status.missing_required)}")
```

### Refactor `install()` to be callable

Extract the install logic so `init` can call it:

```python
def _install_missing(status: SetupStatus) -> None:
    """Install missing required dependencies."""
    if not status.node:
        _install_node()
    if not status.claude:
        _run(["npm", "install", "-g", "@anthropic-ai/claude-code"])
    if not status.worktrunk:
        _install_worktrunk()
```

## Constraints

- macOS only—check `sys.platform` early, exit with message on other platforms
- No network calls in `check_setup()`—just `shutil.which()` checks
- Keep `install` and `doctor` commands working as before (don't break existing users)
- `--yes` flag for non-interactive use

## Done when

```bash
# Fresh repo, no prior loopflow setup
cd /tmp/test-repo && git init

# Single command does everything
lf ops init
# Output:
# Checking dependencies...
#   ✓ Node.js
#   ✗ Claude Code
#   ✓ worktrunk
#
# Missing: claude
# Install missing dependencies? [Y/n] y
#   Installing Claude Code...
#   ✓ Claude Code installed
#
# Creating .lf/...
#   ✓ .lf/config.yaml
#   ✓ .claude/commands/
#
# ✓ Ready! Try 'lf review' or 'lf design'

# Verify
lf ops doctor
# All green

# Non-interactive
lf ops init --yes
# Skips prompts, auto-installs
```

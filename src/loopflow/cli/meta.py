"""Setup and diagnostics commands."""

import platform
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import typer

from loopflow.config import load_config
from loopflow.context import find_worktree_root
from loopflow.launcher import check_claude_available, check_codex_available, check_gemini_available

app = typer.Typer(help="Setup and diagnostics.")

# Prompts installed by `lf ops init`
_BUNDLED_PROMPTS = [
    "review.md",
    "implement.md",
    "design.md",
    "polish.md",
    "debug.md",
    "publish.md",
    "iterate.md",
    "expand.md",
    "reduce.md",
]


@dataclass
class SetupStatus:
    """What's installed and what's missing."""

    node: bool
    claude: bool
    worktrunk: bool
    warp: bool
    cursor: bool
    codex: bool
    gemini: bool
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


def check_setup(repo_root: Path | None = None) -> SetupStatus:
    """Check all dependencies and repo state. Fast (no network)."""
    return SetupStatus(
        node=shutil.which("npm") is not None,
        claude=shutil.which("claude") is not None,
        worktrunk=shutil.which("wt") is not None,
        warp=shutil.which("warp") is not None,
        cursor=shutil.which("cursor") is not None,
        codex=shutil.which("codex") is not None,
        gemini=shutil.which("gemini") is not None,
        has_config=repo_root is not None and (repo_root / ".lf" / "config.yaml").exists(),
        has_prompts=repo_root is not None and (repo_root / ".claude" / "commands").exists(),
    )


def _install_node() -> bool:
    """Attempt to install Node.js via Homebrew on macOS."""
    if platform.system() != "Darwin":
        return False

    if not shutil.which("brew"):
        typer.echo("Homebrew not found. Install from https://brew.sh", err=True)
        return False

    typer.echo("Installing Node.js via Homebrew...")
    result = subprocess.run(["brew", "install", "node"], capture_output=True)
    return result.returncode == 0


def _install_cask(name: str) -> bool:
    """Install a Homebrew cask. Returns success."""
    result = subprocess.run(
        ["brew", "install", "--cask", name],
        capture_output=True,
    )
    return result.returncode == 0


def _install_worktrunk() -> bool:
    """Install worktrunk CLI via Homebrew, with cargo fallback."""
    typer.echo("Installing worktrunk...")
    result = subprocess.run(
        ["brew", "install", "max-sixty/worktrunk/wt"],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return True

    if shutil.which("cargo"):
        typer.echo("Homebrew install failed, trying cargo...")
        result = subprocess.run(
            ["cargo", "install", "worktrunk"],
            capture_output=True,
            text=True,
        )
        return result.returncode == 0

    return False


def _print_setup_status(status: SetupStatus) -> None:
    """Print dependency check results."""
    typer.echo("Checking dependencies...")

    def icon(ok: bool) -> str:
        return "✓" if ok else "✗"

    typer.echo(f"  {icon(status.node)} Node.js")
    typer.echo(f"  {icon(status.claude)} Claude Code")
    typer.echo(f"  {icon(status.worktrunk)} worktrunk")

    if status.missing_required:
        typer.echo(f"\nMissing: {', '.join(status.missing_required)}")


def _install_missing(status: SetupStatus) -> None:
    """Install missing required dependencies."""
    if not status.node:
        typer.echo("  Installing Node.js...")
        if _install_node() and shutil.which("npm"):
            typer.echo("  ✓ Node.js installed")
        else:
            typer.echo("  ✗ Could not install Node.js", err=True)
            raise typer.Exit(1)

    if not status.claude:
        typer.echo("  Installing Claude Code...")
        result = subprocess.run(
            ["npm", "install", "-g", "@anthropic-ai/claude-code"],
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            typer.echo("  ✓ Claude Code installed")
        else:
            typer.echo(f"  ✗ Could not install Claude Code: {result.stderr}", err=True)
            raise typer.Exit(1)

    if not status.worktrunk:
        typer.echo("  Installing worktrunk...")
        if _install_worktrunk() and shutil.which("wt"):
            typer.echo("  ✓ worktrunk installed")
        else:
            typer.echo("  ✗ Could not install worktrunk", err=True)
            raise typer.Exit(1)


def _scaffold_repo(repo_root: Path) -> None:
    """Create .lf/ config and .claude/commands/ prompts."""
    bundled_dir = Path(__file__).parent.parent
    commands_src = bundled_dir / "commands"
    prompts_dir = bundled_dir / "prompts"
    style_template = bundled_dir / "LOOPFLOW_STYLE.md"
    config_template = bundled_dir / "config_template.yaml"

    typer.echo("\nCreating .lf/...")

    # Config
    config_dir = repo_root / ".lf"
    config_dir.mkdir(exist_ok=True)
    config_dst = config_dir / "config.yaml"

    if config_dst.exists():
        typer.echo("  - .lf/config.yaml (already exists)")
    else:
        shutil.copy(config_template, config_dst)
        typer.echo("  ✓ .lf/config.yaml")

    # Style guide
    style_dst = config_dir / "LOOPFLOW_STYLE.md"
    if style_dst.exists():
        typer.echo("  - .lf/LOOPFLOW_STYLE.md (already exists)")
    else:
        shutil.copy(style_template, style_dst)
        typer.echo("  ✓ .lf/LOOPFLOW_STYLE.md")

    # Commit templates
    for template_name in ["COMMIT_MESSAGE.md", "CHECKPOINT_MESSAGE.md"]:
        src = prompts_dir / template_name
        dst = config_dir / template_name
        if dst.exists():
            typer.echo(f"  - .lf/{template_name} (already exists)")
        else:
            shutil.copy(src, dst)
            typer.echo(f"  ✓ .lf/{template_name}")

    # Prompts
    commands_dir = repo_root / ".claude" / "commands"
    commands_dir.mkdir(parents=True, exist_ok=True)
    typer.echo("  ✓ .claude/commands/")

    for prompt_name in _BUNDLED_PROMPTS:
        src = commands_src / prompt_name
        dst = commands_dir / prompt_name
        if not dst.exists():
            shutil.copy(src, dst)


def _install_subset(repo_root: Path, prompts: bool, style: bool) -> None:
    """Install just prompts or style guide (legacy behavior for --prompts/--style flags)."""
    bundled_dir = Path(__file__).parent.parent
    commands_src = bundled_dir / "commands"
    style_template = bundled_dir / "LOOPFLOW_STYLE.md"

    if prompts:
        commands_dir = repo_root / ".claude" / "commands"
        commands_dir.mkdir(parents=True, exist_ok=True)

        for prompt_name in _BUNDLED_PROMPTS:
            src = commands_src / prompt_name
            dst = commands_dir / prompt_name

            if dst.exists():
                typer.echo(f"- .claude/commands/{prompt_name} (already exists)")
            else:
                shutil.copy(src, dst)
                typer.echo(f"✓ Created .claude/commands/{prompt_name}")

    if style:
        lf_dir = repo_root / ".lf"
        lf_dir.mkdir(exist_ok=True)
        style_dst = lf_dir / "LOOPFLOW_STYLE.md"
        if style_dst.exists():
            typer.echo("- .lf/LOOPFLOW_STYLE.md (already exists)")
        else:
            shutil.copy(style_template, style_dst)
            typer.echo("✓ Created .lf/LOOPFLOW_STYLE.md")


@app.command()
def init(
    prompts_only: bool = typer.Option(False, "--prompts", help="Only install prompts"),
    style_only: bool = typer.Option(False, "--style", help="Only install style guide"),
    yes: bool = typer.Option(False, "--yes", "-y", help="Auto-confirm prompts"),
):
    """Initialize repo with loopflow. Checks deps, installs missing, scaffolds config."""
    # macOS only
    if sys.platform != "darwin":
        typer.echo("Error: loopflow requires macOS", err=True)
        raise typer.Exit(1)

    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    # If specific flags, use subset behavior
    if prompts_only or style_only:
        _install_subset(repo_root, prompts=prompts_only, style=style_only)
        return

    # Full init flow
    status = check_setup(repo_root)
    _print_setup_status(status)

    # Handle missing deps
    if status.missing_required:
        if yes or typer.confirm("Install missing dependencies?", default=True):
            _install_missing(status)
            status = check_setup(repo_root)  # recheck
        else:
            typer.echo("\nRun 'lf ops install' to install dependencies manually.")
            raise typer.Exit(1)

    # Scaffold repo
    _scaffold_repo(repo_root)

    # Success message
    typer.echo("\n✓ Ready! Try 'lf review' or 'lf design'")


@app.command()
def version():
    """Show loopflow version."""
    from loopflow import __version__

    typer.echo(f"loopflow {__version__}")


@app.command()
def install():
    """Install loopflow dependencies based on config. macOS only."""
    if platform.system() != "Darwin":
        typer.echo("Error: lf install only supports macOS", err=True)
        typer.echo("Install dependencies manually.", err=True)
        raise typer.Exit(1)

    if not shutil.which("brew"):
        typer.echo("Error: Homebrew not found. Install from https://brew.sh", err=True)
        raise typer.Exit(1)

    # Load config to check what's needed
    repo_root = find_worktree_root()
    config = load_config(repo_root) if repo_root else None
    ide = config.ide if config else None

    # Node.js (required for Claude Code)
    if not shutil.which("npm"):
        typer.echo("Installing Node.js...")
        if _install_node() and shutil.which("npm"):
            typer.echo("✓ Node.js installed")
        else:
            typer.echo("✗ Could not install Node.js", err=True)
            raise typer.Exit(1)
    else:
        typer.echo("✓ Node.js")

    # Claude Code
    if check_claude_available():
        typer.echo("✓ Claude Code")
    else:
        typer.echo("Installing Claude Code...")
        result = subprocess.run(
            ["npm", "install", "-g", "@anthropic-ai/claude-code"],
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            typer.echo("✓ Claude Code installed")
        else:
            typer.echo(f"✗ Could not install Claude Code: {result.stderr}", err=True)

    # Codex
    if check_codex_available():
        typer.echo("✓ Codex")
    else:
        typer.echo("Installing Codex...")
        result = subprocess.run(
            ["npm", "install", "-g", "@openai/codex"],
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            typer.echo("✓ Codex installed")
        else:
            typer.echo(f"✗ Could not install Codex: {result.stderr}", err=True)

    # Gemini CLI
    if check_gemini_available():
        typer.echo("✓ Gemini CLI")
    else:
        typer.echo("Installing Gemini CLI...")
        result = subprocess.run(
            ["npm", "install", "-g", "@google/gemini-cli"],
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            typer.echo("✓ Gemini CLI installed")
        else:
            typer.echo(f"✗ Could not install Gemini CLI: {result.stderr}", err=True)

    # Worktrunk (required for worktree operations)
    if shutil.which("wt"):
        typer.echo("✓ worktrunk")
    else:
        if _install_worktrunk() and shutil.which("wt"):
            typer.echo("✓ worktrunk installed")
        else:
            typer.echo("✗ Could not install worktrunk", err=True)
            raise typer.Exit(1)

    # Warp (if enabled in config, default true)
    if not ide or ide.warp:
        if shutil.which("warp"):
            typer.echo("✓ Warp")
        else:
            typer.echo("Installing Warp...")
            if _install_cask("warp"):
                typer.echo("✓ Warp installed")
            else:
                typer.echo("✗ Could not install Warp", err=True)

    # Cursor (if enabled in config, default true)
    if not ide or ide.cursor:
        if shutil.which("cursor"):
            typer.echo("✓ Cursor")
        else:
            typer.echo("Installing Cursor...")
            if _install_cask("cursor"):
                typer.echo("✓ Cursor installed")
            else:
                typer.echo("✗ Could not install Cursor", err=True)


@app.command()
def doctor():
    """Check loopflow dependencies based on config."""
    all_ok = True

    # Load config to check what's needed
    repo_root = find_worktree_root()
    config = load_config(repo_root) if repo_root else None
    ide = config.ide if config else None

    # Required
    if shutil.which("npm"):
        typer.echo("✓ npm")
    else:
        typer.echo("✗ npm - Install Node.js: https://nodejs.org")
        all_ok = False

    if check_claude_available():
        typer.echo("✓ claude")
    else:
        typer.echo("✗ claude - Run: lf ops install")
        all_ok = False

    if shutil.which("wt"):
        typer.echo("✓ wt")
    else:
        typer.echo("✗ wt - Run: lf ops install")
        all_ok = False

    # IDE tools (based on config)
    if not ide or ide.warp:
        if shutil.which("warp"):
            typer.echo("✓ warp")
        else:
            typer.echo("✗ warp - Run: lf ops install")
            all_ok = False

    if not ide or ide.cursor:
        if shutil.which("cursor"):
            typer.echo("✓ cursor")
        else:
            typer.echo("✗ cursor - Run: lf ops install")
            all_ok = False

    # Optional model backends
    if check_codex_available():
        typer.echo("✓ codex (optional)")
    else:
        typer.echo("- codex (optional): npm install -g @openai/codex")

    if check_gemini_available():
        typer.echo("✓ gemini (optional)")
    else:
        typer.echo("- gemini (optional): npm install -g @google/gemini-cli")

    # Optional: gh for PR creation
    if shutil.which("gh"):
        typer.echo("✓ gh (optional)")
    else:
        typer.echo("- gh (optional): brew install gh")

    raise typer.Exit(0 if all_ok else 1)

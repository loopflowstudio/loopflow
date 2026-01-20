"""Init, install, doctor, and version commands."""

import platform
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import typer

from loopflow.init_check import check_init_status
from loopflow.lf.config import load_config
from loopflow.lf.context import find_worktree_root
from loopflow.lf.launcher import (
    check_claude_available,
    check_codex_available,
    check_gemini_available,
)


@dataclass
class SetupStatus:
    """What's installed and what's missing."""

    node: bool
    claude: bool
    worktrunk: bool

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


def _check_setup() -> SetupStatus:
    """Check required dependencies. Fast (no network)."""
    return SetupStatus(
        node=shutil.which("npm") is not None,
        claude=shutil.which("claude") is not None,
        worktrunk=shutil.which("wt") is not None,
    )


def _get_templates_dir() -> Path:
    """Return path to bundled templates directory."""
    return Path(__file__).parent.parent / "templates"


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


def _install_superpowers(config) -> bool:
    """Clone superpowers skill library if configured and not present."""
    if not config or not config.skill_sources:
        return False

    # Check if superpowers is configured
    sp_source = None
    for source in config.skill_sources:
        if source.name == "superpowers" or source.prefix == "sp":
            sp_source = source
            break

    if not sp_source:
        return False

    # Expand path and check if it exists
    sp_path = Path(sp_source.path).expanduser()
    if sp_path.exists():
        typer.echo("✓ superpowers")
        return True

    # Clone superpowers
    typer.echo("Installing superpowers skill library...")
    result = subprocess.run(
        ["git", "clone", "https://github.com/obra/superpowers", str(sp_path)],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        typer.echo("✓ superpowers installed")
        return True

    typer.echo(f"✗ Could not clone superpowers: {result.stderr}", err=True)
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
    """Create .lf/config.yaml. Commands are built-in and don't need to be copied."""
    templates = _get_templates_dir()

    config_dir = repo_root / ".lf"
    config_dir.mkdir(exist_ok=True)

    config_src = templates / "config.yaml"
    config_dst = config_dir / "config.yaml"
    if config_dst.exists():
        typer.echo("- .lf/config.yaml (already exists)")
    else:
        shutil.copy(config_src, config_dst)
        typer.echo("✓ .lf/config.yaml")


def _install_style(repo_root: Path) -> None:
    """Install style guide files to .lf/."""
    templates = _get_templates_dir()
    lf_dir = repo_root / ".lf"
    lf_dir.mkdir(exist_ok=True)

    for name in ["STYLE.md", "PROMPTS.md"]:
        src = templates / name
        dst = lf_dir / name
        if dst.exists():
            typer.echo(f"- .lf/{name} (already exists)")
        else:
            shutil.copy(src, dst)
            typer.echo(f"✓ Created .lf/{name}")


def register_commands(app: typer.Typer) -> None:
    """Register init, install, doctor, version commands on the app."""

    @app.command()
    def init(
        style: bool = typer.Option(False, "--style", help="Also install style guide templates"),
        yes: bool = typer.Option(False, "--yes", "-y", help="Auto-confirm prompts"),
    ) -> None:
        """Initialize repo with loopflow.

        Creates .lf/config.yaml for pipelines and settings.
        Commands are built-in and work without initialization.
        """
        # macOS only
        if sys.platform != "darwin":
            typer.echo("Error: loopflow requires macOS", err=True)
            raise typer.Exit(1)

        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        # Full init flow
        status = _check_setup()
        _print_setup_status(status)

        # Handle missing deps
        if status.missing_required:
            if yes or typer.confirm("Install missing dependencies?", default=True):
                _install_missing(status)
            else:
                typer.echo("\nRun 'lfops install' to install dependencies manually.")
                raise typer.Exit(1)

        # Scaffold repo
        _scaffold_repo(repo_root)

        # Optional style guide
        if style:
            _install_style(repo_root)

        # Success message
        typer.echo("\n✓ Ready! Run 'lf' to see available tasks.")

    @app.command()
    def install() -> None:
        """Install loopflow dependencies (Claude, Codex, worktrunk, etc)."""
        if platform.system() != "Darwin":
            typer.echo("Error: lfops install only supports macOS", err=True)
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

        # Superpowers skill library (if configured)
        _install_superpowers(config)

    @app.command()
    def doctor() -> None:
        """Check loopflow dependencies and repo status."""
        all_ok = True

        # Load config to check what's needed
        repo_root = find_worktree_root()
        config = load_config(repo_root) if repo_root else None
        ide = config.ide if config else None

        # Repo status
        if repo_root:
            status = check_init_status(repo_root)
            if status.has_commands:
                typer.echo("✓ task files found")
            else:
                typer.echo("- no task files (run: lfops init)")
        else:
            typer.echo("- not in a git repo")

        # Required
        if shutil.which("npm"):
            typer.echo("✓ npm")
        else:
            typer.echo("✗ npm - Install Node.js: https://nodejs.org")
            all_ok = False

        if check_claude_available():
            typer.echo("✓ claude")
        else:
            typer.echo("✗ claude - Run: lfops install")
            all_ok = False

        if shutil.which("wt"):
            typer.echo("✓ wt")
        else:
            typer.echo("✗ wt - Run: lfops install")
            all_ok = False

        # IDE tools (based on config)
        if not ide or ide.warp:
            if shutil.which("warp"):
                typer.echo("✓ warp")
            else:
                typer.echo("✗ warp - Run: lfops install")
                all_ok = False

        if not ide or ide.cursor:
            if shutil.which("cursor"):
                typer.echo("✓ cursor")
            else:
                typer.echo("✗ cursor - Run: lfops install")
                all_ok = False

        # Skill libraries (if configured)
        if config and config.skill_sources:
            for source in config.skill_sources:
                if source.name == "superpowers" or source.prefix == "sp":
                    sp_path = Path(source.path).expanduser()
                    if sp_path.exists():
                        typer.echo("✓ superpowers")
                    else:
                        typer.echo("✗ superpowers - Run: lfops install")
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

    @app.command()
    def version() -> None:
        """Show loopflow version."""
        from loopflow import __version__

        typer.echo(f"loopflow {__version__}")

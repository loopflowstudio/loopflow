"""Setup and diagnostics commands."""

import platform
import shutil
import subprocess
from pathlib import Path

import typer

from loopflow.config import load_config
from loopflow.context import find_worktree_root
from loopflow.launcher import check_claude_available

app = typer.Typer(help="Setup and diagnostics.")


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


@app.command()
def init(
    prompts_only: bool = typer.Option(False, "--prompts", help="Only install prompts"),
    style_only: bool = typer.Option(False, "--style", help="Only install style guide"),
):
    """Initialize a repository with loopflow prompts, style guide, and config."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    # Determine what to install
    install_prompts = prompts_only or not style_only
    install_style = style_only or not prompts_only
    install_config = not prompts_only and not style_only

    # Get bundled assets path
    bundled_dir = Path(__file__).parent.parent
    prompts_dir = bundled_dir / "prompts"
    style_template = bundled_dir / "LOOPFLOW_STYLE.md"
    config_template = bundled_dir / "config_template.yaml"

    # Install prompts to .claude/commands/ (accessible via raw claude too)
    if install_prompts:
        commands_dir = repo_root / ".claude" / "commands"
        commands_dir.mkdir(parents=True, exist_ok=True)

        for prompt_name in ["review.md", "implement.md", "design.md"]:
            src = prompts_dir / prompt_name
            dst = commands_dir / prompt_name

            if dst.exists():
                typer.echo(f"- .claude/commands/{prompt_name} (already exists)")
            else:
                shutil.copy(src, dst)
                typer.echo(f"✓ Created .claude/commands/{prompt_name}")

    # Install style guide to .lf/ (only included in lf sessions, not raw claude/codex)
    if install_style:
        lf_dir = repo_root / ".lf"
        lf_dir.mkdir(exist_ok=True)
        style_dst = lf_dir / "LOOPFLOW_STYLE.md"
        if style_dst.exists():
            typer.echo("- .lf/LOOPFLOW_STYLE.md (already exists)")
        else:
            shutil.copy(style_template, style_dst)
            typer.echo("✓ Created .lf/LOOPFLOW_STYLE.md")

    # Install config
    if install_config:
        config_dir = repo_root / ".lf"
        config_dir.mkdir(exist_ok=True)
        config_dst = config_dir / "config.yaml"

        if config_dst.exists():
            typer.echo("- .lf/config.yaml (already exists)")
        else:
            shutil.copy(config_template, config_dst)
            typer.echo("✓ Created .lf/config.yaml")

    if install_prompts and not prompts_only:
        typer.echo("\nYou can now use:")
        typer.echo("  lf review        # or: claude /review")
        typer.echo("  lf implement")
        typer.echo("  lf design")


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

    # Claude Code (always required)
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
        typer.echo("✗ claude - Run: lf meta install")
        all_ok = False

    # IDE tools (based on config)
    if not ide or ide.warp:
        if shutil.which("warp"):
            typer.echo("✓ warp")
        else:
            typer.echo("✗ warp - Run: lf meta install")
            all_ok = False

    if not ide or ide.cursor:
        if shutil.which("cursor"):
            typer.echo("✓ cursor")
        else:
            typer.echo("✗ cursor - Run: lf meta install")
            all_ok = False

    # Optional: gh for PR creation
    if shutil.which("gh"):
        typer.echo("✓ gh (optional)")
    else:
        typer.echo("- gh (optional): brew install gh")

    raise typer.Exit(0 if all_ok else 1)

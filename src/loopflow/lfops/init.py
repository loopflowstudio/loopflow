"""Doctor and version commands for lfops.

Setup is handled by `lf init` (interactive prompt).
"""

import shutil
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


def register_commands(app: typer.Typer) -> None:
    """Register doctor and version commands on the app."""

    @app.command()
    def doctor() -> None:
        """Check loopflow dependencies and repo status."""
        all_ok = True

        # Load config
        repo_root = find_worktree_root()
        config = load_config(repo_root) if repo_root else None

        # Repo status
        if repo_root:
            status = check_init_status(repo_root)
            if status.has_commands:
                typer.echo("✓ task files found")
            else:
                typer.echo("- no task files (run: lf init)")
        else:
            typer.echo("- not in a git repo")

        # Required: only worktrunk
        if shutil.which("wt"):
            typer.echo("✓ wt")
        else:
            typer.echo("✗ wt - Run: lf init")
            all_ok = False

        # Optional: coding agents
        if shutil.which("npm"):
            typer.echo("✓ npm")
        else:
            typer.echo("- npm: brew install node")

        if check_claude_available():
            typer.echo("✓ claude")
        else:
            typer.echo("- claude: lf init")

        if check_codex_available():
            typer.echo("✓ codex")
        else:
            typer.echo("- codex: npm install -g @openai/codex")

        if check_gemini_available():
            typer.echo("✓ gemini")
        else:
            typer.echo("- gemini: npm install -g @google/gemini-cli")

        # Optional: IDE/terminals
        if shutil.which("warp"):
            typer.echo("✓ warp")
        else:
            typer.echo("- warp: brew install --cask warp")

        if shutil.which("cursor"):
            typer.echo("✓ cursor")
        else:
            typer.echo("- cursor: brew install --cask cursor")

        # Optional: skill libraries
        if config and config.skill_sources:
            for source in config.skill_sources:
                if source.name == "superpowers" or source.prefix == "sp":
                    sp_path = Path(source.path).expanduser()
                    if sp_path.exists():
                        typer.echo("✓ superpowers")
                    else:
                        url = "https://github.com/obra/superpowers"
                        typer.echo(f"- superpowers: git clone {url} ~/.superpowers")

        # Optional: gh for PR creation
        if shutil.which("gh"):
            typer.echo("✓ gh")
        else:
            typer.echo("- gh: brew install gh")

        raise typer.Exit(0 if all_ok else 1)

    @app.command()
    def version() -> None:
        """Show loopflow version."""
        from loopflow import __version__

        typer.echo(f"loopflow {__version__}")

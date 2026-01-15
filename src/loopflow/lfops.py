"""lfops: Loopflow meta operations CLI."""

import os
import platform
import shutil
import signal
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

import typer

from loopflow.config import load_config
from loopflow.context import find_worktree_root
from loopflow.init_check import check_init_status
from loopflow.launcher import check_claude_available, check_codex_available, check_gemini_available
from loopflow.logging import get_log_dir
from loopflow.maestro.db import (
    DEFAULT_DB_PATH,
    delete_session,
    load_sessions,
    update_session_status,
)
from loopflow.maestro.session import SessionStatus

app = typer.Typer(help="Loopflow meta operations")

# Starter prompts installed by default
_STARTER_PROMPTS = [
    "design.md",
    "implement.md",
    "review.md",
    "debug.md",
    "polish.md",
    "iterate.md",
]


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
    return Path(__file__).parent / "templates"


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


def _scaffold_repo(repo_root: Path, all_prompts: bool = False) -> None:
    """Create .lf/ config and .claude/commands/ prompts."""
    templates = _get_templates_dir()
    prompts_dir = Path(__file__).parent / "prompts"

    typer.echo("\nCreating .lf/...")

    # Config
    config_dir = repo_root / ".lf"
    config_dir.mkdir(exist_ok=True)

    config_src = templates / "config.yaml"
    config_dst = config_dir / "config.yaml"
    if config_dst.exists():
        typer.echo("  - .lf/config.yaml (already exists)")
    else:
        shutil.copy(config_src, config_dst)
        typer.echo("  ✓ .lf/config.yaml")

    # Style guide and PROMPTS.md
    for name in ["STYLE.md", "PROMPTS.md"]:
        src = templates / name
        dst = config_dir / name
        if dst.exists():
            typer.echo(f"  - .lf/{name} (already exists)")
        else:
            shutil.copy(src, dst)
            typer.echo(f"  ✓ .lf/{name}")

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

    if all_prompts:
        prompt_files = list((templates / "commands").glob("*.md"))
    else:
        prompt_files = [templates / "commands" / name for name in _STARTER_PROMPTS]

    for src in prompt_files:
        dst = commands_dir / src.name
        if not dst.exists():
            shutil.copy(src, dst)


def _install_subset(repo_root: Path, prompts: bool, style: bool, all_prompts: bool = False) -> None:
    """Install just prompts or style guide (legacy behavior for --prompts/--style flags)."""
    templates = _get_templates_dir()

    if prompts:
        commands_dir = repo_root / ".claude" / "commands"
        commands_dir.mkdir(parents=True, exist_ok=True)

        if all_prompts:
            prompt_files = list((templates / "commands").glob("*.md"))
        else:
            prompt_files = [templates / "commands" / name for name in _STARTER_PROMPTS]

        for src in prompt_files:
            dst = commands_dir / src.name
            if dst.exists():
                typer.echo(f"- .claude/commands/{src.name} (already exists)")
            else:
                shutil.copy(src, dst)
                typer.echo(f"✓ Created .claude/commands/{src.name}")

    if style:
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


@app.command()
def init(
    prompts_only: bool = typer.Option(False, "--prompts", help="Only install prompts"),
    style_only: bool = typer.Option(False, "--style", help="Only install style guide"),
    all_prompts: bool = typer.Option(False, "--all", help="Install all prompts, not just starter set"),
    yes: bool = typer.Option(False, "--yes", "-y", help="Auto-confirm prompts"),
) -> None:
    """Initialize repo with loopflow."""
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
        _install_subset(repo_root, prompts=prompts_only, style=style_only, all_prompts=all_prompts)
        return

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
    _scaffold_repo(repo_root, all_prompts=all_prompts)

    # Success message
    typer.echo("\n✓ Ready! Try 'lf review' or 'lf design'")


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


def _format_time_ago(started_at: datetime) -> str:
    """Format time difference as '2m ago', '5h ago', etc."""
    delta = datetime.now() - started_at
    seconds = int(delta.total_seconds())

    if seconds < 60:
        return f"{seconds}s ago"
    elif seconds < 3600:
        return f"{seconds // 60}m ago"
    elif seconds < 86400:
        return f"{seconds // 3600}h ago"
    else:
        return f"{seconds // 86400}d ago"


@app.command()
def status(
    all_repos: bool = typer.Option(False, "--all", "-a", help="Show sessions from all repos"),
) -> None:
    """Show running sessions."""
    repo = None if all_repos else find_worktree_root()
    sessions = load_sessions(DEFAULT_DB_PATH, repo=repo)

    if not sessions:
        typer.echo("No running sessions")
        raise typer.Exit(0)

    # Print header
    typer.echo(f"{'ID':<10} {'TASK':<14} {'WORKTREE':<24} {'STATUS':<10} {'STARTED'}")

    # Print sessions
    for session in sessions:
        worktree_name = session.worktree.name
        time_ago = _format_time_ago(session.started_at)
        typer.echo(
            f"{session.id[:8]:<10} {session.task:<14} {worktree_name:<24} {session.status.value:<10} {time_ago}"
        )


def _resolve_session(sessions, prefix: str):
    matches = [session for session in sessions if session.id.startswith(prefix)]
    if not matches:
        typer.echo(f"Error: No session matching '{prefix}'", err=True)
        raise typer.Exit(1)
    if len(matches) > 1:
        ids = ", ".join(session.id[:8] for session in matches)
        typer.echo(f"Error: Ambiguous session id '{prefix}': {ids}", err=True)
        raise typer.Exit(1)
    return matches[0]


@app.command()
def stop(
    session_id: str = typer.Argument(help="Session id (prefix ok)"),
    all_repos: bool = typer.Option(False, "--all", "-a", help="Search sessions from all repos"),
    force: bool = typer.Option(False, "--force", help="Send SIGKILL instead of SIGTERM"),
) -> None:
    """Stop a running session."""
    repo = None if all_repos else find_worktree_root()
    sessions = load_sessions(DEFAULT_DB_PATH, repo=repo, include_completed=True)
    session = _resolve_session(sessions, session_id)

    if session.status not in (SessionStatus.RUNNING, SessionStatus.WAITING):
        typer.echo(f"Session {session.id[:8]} is not running")
        raise typer.Exit(0)

    if not session.pid:
        typer.echo(f"Error: Session {session.id[:8]} has no PID to stop", err=True)
        raise typer.Exit(1)

    try:
        os.kill(session.pid, signal.SIGKILL if force else signal.SIGTERM)
    except OSError as e:
        typer.echo(f"Error: Failed to stop session {session.id[:8]}: {e}", err=True)
        raise typer.Exit(1)

    update_session_status(DEFAULT_DB_PATH, session.id, SessionStatus.ERROR)
    typer.echo(f"Stopped session {session.id[:8]}")


@app.command()
def prune(
    all_repos: bool = typer.Option(False, "--all", "-a", help="Prune sessions from all repos"),
) -> None:
    """Remove completed sessions and their logs."""
    repo = None if all_repos else find_worktree_root()
    sessions = load_sessions(DEFAULT_DB_PATH, repo=repo, include_completed=True)

    removed = 0
    for session in sessions:
        if session.status in (SessionStatus.RUNNING, SessionStatus.WAITING):
            continue

        log_dir = get_log_dir(Path(session.worktree))
        for suffix in (".log", ".jsonl"):
            log_path = log_dir / f"{session.id}{suffix}"
            if log_path.exists():
                try:
                    log_path.unlink()
                except OSError:
                    pass

        if delete_session(DEFAULT_DB_PATH, session.id):
            removed += 1

    typer.echo(f"Pruned {removed} sessions")


def main() -> None:
    """Entry point for lfops command."""
    if len(sys.argv) == 1:
        sys.argv.append("doctor")
    app()


if __name__ == "__main__":
    main()

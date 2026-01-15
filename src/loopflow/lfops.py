"""lfops: Loopflow operations CLI."""

import json
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
from loopflow.design import clear_design_artifacts
from loopflow.git import GitError, find_main_repo, get_current_branch, has_upstream, open_pr
from loopflow.init_check import check_init_status
from loopflow.launcher import check_claude_available, check_codex_available, check_gemini_available
from loopflow.llm_http import generate_commit_message, generate_commit_message_from_diff, generate_pr_message
from loopflow.logging import get_log_dir
from loopflow.lfd.db import delete_session, load_sessions, update_session_status
from loopflow.lfd.models import SessionStatus
from loopflow.worktrees import get_path

app = typer.Typer(help="Loopflow operations")

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


@dataclass
class Issue:
    """A repository inconsistency that can be fixed."""

    kind: str
    description: str
    fix_cmd: str


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
    sessions = load_sessions(repo=str(repo) if repo else None)

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
    sessions = load_sessions(repo=str(repo) if repo else None)
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

    update_session_status(session.id, SessionStatus.ERROR)
    typer.echo(f"Stopped session {session.id[:8]}")


@app.command()
def prune(
    all_repos: bool = typer.Option(False, "--all", "-a", help="Prune sessions from all repos"),
) -> None:
    """Remove completed sessions and their logs."""
    repo = None if all_repos else find_worktree_root()
    sessions = load_sessions(repo=str(repo) if repo else None)

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

        if delete_session(session.id):
            removed += 1

    typer.echo(f"Pruned {removed} sessions")


# =============================================================================
# PR and landing operations (merged from lfpr)
# =============================================================================


def _add_commit_push(repo_root: Path, push: bool = True) -> bool:
    """Add, commit (with generated message), and optionally push. Returns True if committed."""
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if not result.stdout.strip():
        if push:
            typer.echo("Pushing...")
            subprocess.run(["git", "push"], cwd=repo_root, check=True)
        return False

    typer.echo("Staging changes...")
    subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)

    typer.echo("Generating commit message...")
    message = generate_commit_message(repo_root)
    commit_msg = message.title
    if message.body:
        commit_msg += f"\n\n{message.body}"

    typer.echo(f"Committing: {message.title}")
    subprocess.run(["git", "commit", "-m", commit_msg], cwd=repo_root, check=True)

    if push:
        typer.echo("Pushing...")
        subprocess.run(["git", "push"], cwd=repo_root, check=True)

    return True


def _get_default_branch(repo_root: Path) -> str:
    result = subprocess.run(
        ["git", "symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip().split("/", 1)[-1]
    return "main"


def _resolve_base_ref(repo_root: Path, base_branch: str) -> str:
    origin_ref = f"origin/{base_branch}"
    result = subprocess.run(
        ["git", "rev-parse", "--verify", origin_ref],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return origin_ref
    return base_branch


def _get_diff(repo_root: Path, base_ref: str) -> str:
    result = subprocess.run(
        ["git", "diff", f"{base_ref}...HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    return result.stdout if result.returncode == 0 else ""


def _clear_design_and_push(repo_root: Path) -> bool:
    """Delete .design/* contents, commit, push. Returns True if changes made."""
    design_dir = repo_root / ".design"
    if not design_dir.exists():
        return False

    files = list(design_dir.glob("*"))
    if not files:
        return False

    for f in files:
        if f.is_file():
            f.unlink()
        else:
            shutil.rmtree(f)

    subprocess.run(["git", "add", "-A", str(design_dir)], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", "clear .design/"], cwd=repo_root, check=True)
    subprocess.run(["git", "push"], cwd=repo_root, check=True)
    return True


def _sync_main_repo(main_repo: Path, base_branch: str) -> bool:
    """Sync main repo to latest. Returns False if sync failed."""
    result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "HEAD"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    current = result.stdout.strip()

    result = subprocess.run(
        ["git", "fetch", "origin", base_branch],
        cwd=main_repo,
        capture_output=True,
    )
    if result.returncode != 0:
        return False

    if current == base_branch:
        result = subprocess.run(
            ["git", "pull", "--ff-only"],
            cwd=main_repo,
            capture_output=True,
        )
        return result.returncode == 0
    else:
        result = subprocess.run(
            ["git", "checkout", base_branch],
            cwd=main_repo,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            return False
        result = subprocess.run(
            ["git", "pull", "--ff-only"],
            cwd=main_repo,
            capture_output=True,
        )
        return result.returncode == 0


def _remove_worktree(main_repo: Path, branch: str, worktree_path: Path) -> None:
    """Remove worktree and branch. Uses wt for events, falls back to git if needed."""
    # Try wt first (emits events for Maestro)
    result = subprocess.run(
        ["wt", "-C", str(main_repo), "remove", branch],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return

    # wt failed - fall back to git directly (handles "main already used" errors)
    subprocess.run(
        ["git", "worktree", "remove", "--force", str(worktree_path)],
        cwd=main_repo,
        capture_output=True,
    )
    subprocess.run(
        ["git", "branch", "-D", branch],
        cwd=main_repo,
        capture_output=True,
    )


def _squash_commits(repo_root: Path, base_ref: str, commit_msg: str) -> None:
    original_head = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()

    subprocess.run(["git", "reset", "--soft", base_ref], cwd=repo_root, check=True)
    design_dir = repo_root / ".design"
    if design_dir.exists():
        subprocess.run(["git", "add", "-A", str(design_dir)], cwd=repo_root, check=False)

    staged = subprocess.run(
        ["git", "diff", "--cached", "--quiet"],
        cwd=repo_root,
    )
    if staged.returncode == 0:
        subprocess.run(["git", "reset", "--hard", original_head], cwd=repo_root, check=True)
        typer.echo("Error: Nothing to land after squash", err=True)
        raise typer.Exit(1)

    subprocess.run(["git", "commit", "-m", commit_msg], cwd=repo_root, check=True)


def _get_existing_pr_url(repo_root: Path) -> str | None:
    """Check if a PR exists for current branch. Returns URL if exists, None otherwise."""
    result = subprocess.run(
        ["gh", "pr", "view", "--json", "url", "-q", ".url"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip()
    return None


def _update_pr(repo_root: Path, title: str, body: str) -> str:
    """Update existing PR title and body. Returns URL."""
    subprocess.run(
        ["git", "push"],
        cwd=repo_root,
        capture_output=True,
    )
    result = subprocess.run(
        ["gh", "pr", "edit", "--title", title, "--body", body],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "Failed to update PR")
    return _get_existing_pr_url(repo_root) or ""


@app.command("pr")
def pr(
    add: bool = typer.Option(False, "-a", "--add", help="Add, commit, and push changes first"),
) -> None:
    """Create or update a GitHub PR, then open it in browser."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not shutil.which("gh"):
        typer.echo("Error: 'gh' CLI not found. Install with: brew install gh", err=True)
        raise typer.Exit(1)

    if add:
        _add_commit_push(repo_root)

    # Check if PR already exists
    existing_url = _get_existing_pr_url(repo_root)

    if existing_url:
        typer.echo("Updating existing PR...")
        message = generate_pr_message(repo_root)
        typer.echo(f"\n{message.title}\n")
        typer.echo(message.body)
        typer.echo("")
        try:
            pr_url = _update_pr(repo_root, title=message.title, body=message.body)
        except GitError as e:
            typer.echo(f"Error: {e}", err=True)
            raise typer.Exit(1)
        typer.echo(f"Updated: {pr_url}")
    else:
        typer.echo("Creating PR...")
        message = generate_pr_message(repo_root)
        typer.echo(f"\n{message.title}\n")
        typer.echo(message.body)
        typer.echo("")
        try:
            pr_url = open_pr(repo_root, title=message.title, body=message.body)
        except GitError as e:
            typer.echo(f"Error: {e}", err=True)
            raise typer.Exit(1)
        typer.echo(f"Created: {pr_url}")

    subprocess.run(["open", pr_url])


@app.command()
def land(
    worktree: str = typer.Option(None, "-w", "--worktree", help="Target worktree by name"),
    local: bool = typer.Option(None, "-l", "--local/--gh", help="Local merge (no PR) vs GitHub PR merge"),
    create_pr: bool = typer.Option(False, "-c", "--create-pr", help="Create PR and merge in one step"),
    strict: bool = typer.Option(False, "-s", "--strict", help="Error if uncommitted/unpushed changes exist"),
) -> None:
    """Squash-merge branch to main and clean up.

    By default, stages, commits, and pushes any pending changes before landing.
    Use --strict to require clean state (error if uncommitted/unpushed).

    Default: uses gh pr merge (requires PR via lfops pr).
    With --local: local merge + push (no PR needed).
    With --create-pr: create PR and immediately merge.
    Config: set `land: local` in .lf/config.yaml to default to --local.
    """
    main_repo = find_main_repo()
    config = load_config(main_repo) if main_repo else None
    use_local = local if local is not None else (config and config.land == "local")

    if use_local:
        _land_local(strict, worktree)
    else:
        _land_pr(strict, worktree, create_pr=create_pr)


def _land_pr(strict: bool, worktree: str | None, create_pr: bool = False) -> None:
    """Land via GitHub PR merge."""
    if worktree:
        main_repo = find_main_repo()
        if not main_repo:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)
        repo_root = get_path(main_repo, worktree)
        if not repo_root.exists():
            typer.echo(f"Error: Worktree '{worktree}' not found", err=True)
            raise typer.Exit(1)
    else:
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)
        main_repo = find_main_repo(repo_root)
        if not main_repo:
            typer.echo("Error: Could not find main repository", err=True)
            raise typer.Exit(1)

    if not shutil.which("gh"):
        typer.echo("Error: 'gh' CLI not found. Install with: brew install gh", err=True)
        raise typer.Exit(1)

    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    branch = result.stdout.strip()

    if not branch:
        typer.echo("Error: Detached HEAD", err=True)
        raise typer.Exit(1)

    # Handle uncommitted changes
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        if strict:
            typer.echo("Error: Uncommitted changes (use without --strict to auto-commit)", err=True)
            raise typer.Exit(1)
        _add_commit_push(repo_root, push=False)

    # Ensure branch is pushed
    result = subprocess.run(
        ["git", "rev-parse", "@{u}"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    has_upstream_branch = result.returncode == 0

    if has_upstream_branch:
        result = subprocess.run(
            ["git", "rev-list", "@{u}..HEAD", "--count"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        unpushed = int(result.stdout.strip()) if result.returncode == 0 else 0
        if unpushed > 0:
            if strict:
                typer.echo("Error: Unpushed commits (use without --strict to auto-push)", err=True)
                raise typer.Exit(1)
            typer.echo("Pushing to origin...")
            subprocess.run(["git", "push"], cwd=repo_root, check=True)
    else:
        if strict:
            typer.echo("Error: Branch not pushed (use without --strict to auto-push)", err=True)
            raise typer.Exit(1)
        typer.echo("Pushing to origin...")
        subprocess.run(["git", "push", "-u", "origin", branch], cwd=repo_root, check=True)

    # Get PR info (or create PR if --create-pr)
    result = subprocess.run(
        ["gh", "pr", "view", "--json", "number,title,body,baseRefName"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        if create_pr:
            typer.echo("Creating PR...")
            message = generate_pr_message(repo_root)
            try:
                pr_url = open_pr(repo_root, title=message.title, body=message.body)
            except GitError as e:
                typer.echo(f"Error creating PR: {e}", err=True)
                raise typer.Exit(1)
            typer.echo(f"Created: {pr_url}")
            # Re-fetch to get the PR number
            result = subprocess.run(
                ["gh", "pr", "view", "--json", "number,title,body,baseRefName"],
                cwd=repo_root,
                capture_output=True,
                text=True,
            )
            if result.returncode != 0:
                typer.echo("Error: Could not get PR info after creation", err=True)
                raise typer.Exit(1)
            pr_data = json.loads(result.stdout)
            pr_number = pr_data.get("number")
            title = message.title
            body = message.body
            base_branch = _get_default_branch(main_repo)
        else:
            typer.echo("Error: No PR found. Run 'lfops pr create' first, or use --local or --create-pr.", err=True)
            raise typer.Exit(1)
    else:
        pr_data = json.loads(result.stdout)
        pr_number = pr_data.get("number")
        title = pr_data.get("title", "").strip()
        body = pr_data.get("body", "").strip()
        base_branch = pr_data.get("baseRefName", "main").strip()

    if not title:
        typer.echo("Error: PR has no title", err=True)
        raise typer.Exit(1)

    if branch == base_branch:
        typer.echo(f"Error: Cannot land {branch} onto itself", err=True)
        raise typer.Exit(1)

    # Clear .design before merge so it never touches main
    if _clear_design_and_push(repo_root):
        typer.echo("Cleared .design/")

    # Use gh pr merge to squash-merge on GitHub (marks PR as merged, not closed)
    typer.echo(f"Merging PR: {title}")
    merge_cmd = ["gh", "pr", "merge", branch, "--squash", "--delete-branch", "--subject", title]
    if body:
        merge_cmd.extend(["--body", body])
    result = subprocess.run(merge_cmd, cwd=repo_root, capture_output=True, text=True)
    if result.returncode != 0:
        error_msg = result.stderr.strip() or result.stdout.strip() or "merge failed"
        typer.echo(f"Error: {error_msg}", err=True)
        raise typer.Exit(1)

    # Verify the PR was actually merged (not just closed)
    # Use PR number since branch may have been deleted by --delete-branch
    if pr_number:
        result = subprocess.run(
            ["gh", "pr", "view", str(pr_number), "--json", "state", "-q", ".state"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        pr_state = result.stdout.strip().upper() if result.returncode == 0 else ""
        if pr_state and pr_state != "MERGED":
            typer.echo(f"Warning: PR state is '{pr_state}' (expected 'MERGED')", err=True)
            typer.echo("The PR may have been closed without merging. Check GitHub.", err=True)
            raise typer.Exit(1)

    # Sync main repo to get the merged changes (best-effort after merge)
    if not _sync_main_repo(main_repo, base_branch):
        typer.echo(f"Warning: Could not sync {base_branch}. Run manually:", err=True)
        typer.echo(f"  cd {main_repo} && git fetch origin && git checkout {base_branch} && git pull", err=True)

    # Clean up worktree or local branch (best-effort)
    was_in_worktree = repo_root != main_repo
    if was_in_worktree:
        try:
            _remove_worktree(main_repo, branch, repo_root)
        except Exception:
            typer.echo(f"Warning: Could not remove worktree. Run manually:", err=True)
            typer.echo(f"  wt remove {branch}", err=True)
    else:
        subprocess.run(["git", "branch", "-D", branch], cwd=main_repo, capture_output=True)

    typer.echo(f"Landed {branch} onto {base_branch}.")

    if was_in_worktree:
        typer.echo(str(main_repo))


def _land_local(strict: bool, worktree: str | None) -> None:
    """Land locally without PR (squash-merge + push)."""
    if worktree:
        main_repo = find_main_repo()
        if not main_repo:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)
        repo_root = get_path(main_repo, worktree)
        if not repo_root.exists():
            typer.echo(f"Error: Worktree '{worktree}' not found", err=True)
            raise typer.Exit(1)
    else:
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)
        main_repo = find_main_repo(repo_root) or repo_root

    # Handle uncommitted changes
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        if strict:
            typer.echo("Error: Uncommitted changes (use without --strict to auto-commit)", err=True)
            raise typer.Exit(1)
        _add_commit_push(repo_root, push=False)

    branch = get_current_branch(repo_root)
    if not branch:
        typer.echo("Error: Detached HEAD", err=True)
        raise typer.Exit(1)

    base_branch = _get_default_branch(main_repo)
    if branch == base_branch:
        typer.echo(f"Error: Cannot land {branch} onto itself", err=True)
        raise typer.Exit(1)

    # Fetch base branch
    subprocess.run(["git", "fetch", "origin", base_branch], cwd=repo_root, check=False)

    # Check for changes
    base_ref = _resolve_base_ref(repo_root, base_branch)
    diff = _get_diff(repo_root, base_ref)
    if not diff.strip():
        typer.echo("Error: No changes to land", err=True)
        raise typer.Exit(1)

    # Generate commit message
    typer.echo("Generating commit message...")
    message = generate_commit_message_from_diff(repo_root, diff)
    commit_msg = message.title
    if message.body:
        commit_msg += f"\n\n{message.body}"

    # Squash commits on the branch
    _squash_commits(repo_root, base_ref, commit_msg)

    # Check main repo is clean
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    tracked_changes = [
        line for line in result.stdout.strip().split("\n") if line and not line.startswith("??")
    ]
    if tracked_changes:
        typer.echo("Error: Main repo has uncommitted changes", err=True)
        raise typer.Exit(1)

    # Checkout and reset main to origin
    typer.echo(f"Checking out {base_branch}...")
    result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "HEAD"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    current_branch = result.stdout.strip()

    if current_branch != base_branch:
        result = subprocess.run(
            ["git", "checkout", base_branch],
            cwd=main_repo,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            typer.echo(f"Error: Could not checkout {base_branch} in main repo", err=True)
            typer.echo(f"  {result.stderr.strip()}", err=True)
            raise typer.Exit(1)

    subprocess.run(["git", "reset", "--hard", f"origin/{base_branch}"], cwd=main_repo, check=True)

    # Fetch and merge the branch
    subprocess.run(["git", "fetch", "origin", branch], cwd=main_repo, check=False)

    # Try to merge from origin first (if pushed), otherwise from local worktree
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"origin/{branch}"],
        cwd=main_repo,
        capture_output=True,
    )
    if result.returncode == 0:
        merge_ref = f"origin/{branch}"
    else:
        # Branch not pushed, merge from worktree path
        merge_ref = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()

    result = subprocess.run(
        ["git", "merge", "--squash", merge_ref],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        typer.echo(f"Error: Merge failed.\n{result.stderr}", err=True)
        raise typer.Exit(1)

    # Clear .design artifacts
    if clear_design_artifacts(main_repo):
        design_dir = main_repo / ".design"
        if design_dir.exists():
            subprocess.run(["git", "add", "-A", str(design_dir)], cwd=main_repo, check=True)
        typer.echo("Removed .design contents")

    # Check there's something to commit
    result = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=main_repo)
    if result.returncode == 0:
        typer.echo(f"Nothing to land - {branch} has no changes relative to {base_branch}.", err=True)
        raise typer.Exit(1)

    # Commit and push
    typer.echo(f"Committing: {message.title}")
    subprocess.run(["git", "commit", "-m", commit_msg], cwd=main_repo, check=True)
    subprocess.run(["git", "push"], cwd=main_repo, check=True)

    # Delete remote branch if it exists
    subprocess.run(
        ["git", "push", "origin", "--delete", branch],
        cwd=main_repo,
        capture_output=True,
    )

    # Clean up worktree/branch (best-effort after push)
    was_in_worktree = repo_root != main_repo
    if was_in_worktree:
        try:
            _remove_worktree(main_repo, branch, repo_root)
        except Exception:
            typer.echo(f"Warning: Could not remove worktree. Run manually:", err=True)
            typer.echo(f"  wt remove {branch}", err=True)
    else:
        subprocess.run(["git", "branch", "-D", branch], cwd=main_repo, capture_output=True)

    typer.echo(f"Landed {branch} onto {base_branch}.")

    if was_in_worktree:
        typer.echo(str(main_repo))


@app.command()
def commit(
    push: bool = typer.Option(False, "-p", "--push", help="Push after committing"),
    add: bool = typer.Option(True, "-a/-A", "--add/--no-add", help="Stage all changes before committing"),
) -> None:
    """Generate commit message from diff and commit."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if not result.stdout.strip():
        typer.echo("Nothing to commit", err=True)
        raise typer.Exit(0)

    if add:
        subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)

    staged = subprocess.run(
        ["git", "diff", "--cached", "--quiet"],
        cwd=repo_root,
    )
    if staged.returncode == 0:
        typer.echo("Nothing staged to commit", err=True)
        raise typer.Exit(0)

    typer.echo("Generating commit message...")
    try:
        message = generate_commit_message(repo_root)
    except Exception as e:
        typer.echo(f"Error generating commit message: {e}", err=True)
        raise typer.Exit(1)

    commit_msg = message.title
    if message.body:
        commit_msg += f"\n\n{message.body}"

    result = subprocess.run(
        ["git", "commit", "-m", commit_msg],
        cwd=repo_root,
    )
    if result.returncode != 0:
        typer.echo("Commit failed", err=True)
        raise typer.Exit(1)

    typer.echo(f"Committed: {message.title}")

    if push:
        if has_upstream(repo_root):
            result = subprocess.run(["git", "push"], cwd=repo_root)
            if result.returncode == 0:
                typer.echo("Pushed to origin")
            else:
                typer.echo("Push failed", err=True)
                raise typer.Exit(1)
        else:
            typer.echo("No upstream branch, skipping push", err=True)


def _detect_issues(repo: Path) -> list[Issue]:
    """Find inconsistencies in repo state."""
    issues = []

    # Get default branch first (needed for all checks)
    result = subprocess.run(
        ["git", "symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    default_branch = result.stdout.strip().split("/")[-1] if result.returncode == 0 else "main"

    # 1. Stale local default branch - check if origin is ahead
    result = subprocess.run(
        ["git", "rev-list", f"{default_branch}..origin/{default_branch}", "--count"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        behind = int(result.stdout.strip() or "0")
        if behind > 0:
            s = "s" if behind > 1 else ""
            issues.append(Issue(
                kind="stale_main",
                description=f"{default_branch} is {behind} commit{s} behind origin/{default_branch}",
                fix_cmd=f"git checkout {default_branch} && git pull --ff-only",
            ))

    # 2. Orphaned local branches - local branch with no remote and no worktree
    result = subprocess.run(
        ["git", "branch", "--format=%(refname:short)"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    local_branches = [b.strip() for b in result.stdout.strip().split("\n") if b.strip()]

    # Get worktree branches
    result = subprocess.run(
        ["git", "worktree", "list", "--porcelain"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    worktree_branches = set()
    for line in result.stdout.split("\n"):
        if line.startswith("branch refs/heads/"):
            worktree_branches.add(line.split("/")[-1])

    for branch in local_branches:
        if branch == default_branch:
            continue
        if branch in worktree_branches:
            continue

        # Check if remote exists
        result = subprocess.run(
            ["git", "rev-parse", "--verify", f"origin/{branch}"],
            cwd=repo,
            capture_output=True,
        )
        has_remote = result.returncode == 0

        if not has_remote:
            # Check if merged into default branch
            result = subprocess.run(
                ["git", "merge-base", "--is-ancestor", branch, default_branch],
                cwd=repo,
                capture_output=True,
            )
            is_merged = result.returncode == 0

            if is_merged:
                issues.append(Issue(
                    kind="orphan_branch",
                    description=f"'{branch}' has no remote and no worktree (merged)",
                    fix_cmd=f"git branch -D {branch}",
                ))

    # 3. Orphaned worktrees - git worktree prune --dry-run
    result = subprocess.run(
        ["git", "worktree", "prune", "--dry-run"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        issues.append(Issue(
            kind="orphan_worktree",
            description="stale worktree entries in .git/worktrees",
            fix_cmd="git worktree prune",
        ))

    # 4. Stale remote refs - git remote prune origin --dry-run
    result = subprocess.run(
        ["git", "remote", "prune", "origin", "--dry-run"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip() and "would prune" in result.stdout:
        # Count prunable refs
        prune_count = result.stdout.count("would prune")
        s = "s" if prune_count > 1 else ""
        issues.append(Issue(
            kind="stale_ref",
            description=f"{prune_count} stale remote tracking ref{s}",
            fix_cmd="git remote prune origin",
        ))

    return issues


def _fix_issue(repo: Path, issue: Issue) -> bool:
    """Apply fix. Returns True if successful."""
    if issue.kind == "stale_main":
        # Parse branch from fix_cmd: "git checkout <branch> && git pull --ff-only"
        branch = issue.fix_cmd.split()[2]
        result = subprocess.run(["git", "checkout", branch], cwd=repo, capture_output=True)
        if result.returncode != 0:
            return False
        result = subprocess.run(["git", "pull", "--ff-only"], cwd=repo, capture_output=True)
        return result.returncode == 0

    elif issue.kind == "orphan_branch":
        branch = issue.fix_cmd.split()[-1]
        result = subprocess.run(["git", "branch", "-D", branch], cwd=repo, capture_output=True)
        return result.returncode == 0

    elif issue.kind == "orphan_worktree":
        result = subprocess.run(["git", "worktree", "prune"], cwd=repo, capture_output=True)
        return result.returncode == 0

    elif issue.kind == "stale_ref":
        result = subprocess.run(["git", "remote", "prune", "origin"], cwd=repo, capture_output=True)
        return result.returncode == 0

    return False


@app.command()
def recover(
    fix: bool = typer.Option(False, "--fix", "-f", help="Apply fixes (default: dry-run)"),
) -> None:
    """Detect and fix repository inconsistencies."""
    repo = find_main_repo()
    if not repo:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    # Fetch to get latest remote state
    subprocess.run(["git", "fetch", "origin"], cwd=repo, capture_output=True)

    issues = _detect_issues(repo)

    if not issues:
        typer.echo("No issues found")
        raise typer.Exit(0)

    if fix:
        fixed = 0
        for issue in issues:
            typer.echo(f"Fixing {issue.kind}: {issue.description}")
            if _fix_issue(repo, issue):
                fixed += 1
            else:
                typer.echo(f"  Failed: {issue.fix_cmd}", err=True)
        typer.echo(f"\nFixed {fixed}/{len(issues)} issues")
    else:
        typer.echo(f"Found {len(issues)} issue{'s' if len(issues) > 1 else ''}:\n")
        for issue in issues:
            typer.echo(f"  {issue.kind}: {issue.description}")
            typer.echo(f"    fix: {issue.fix_cmd}\n")
        typer.echo("Run 'lfops recover --fix' to apply fixes.")


def main() -> None:
    """Entry point for lfops command."""
    if len(sys.argv) == 1:
        sys.argv.append("doctor")
    app()


if __name__ == "__main__":
    main()

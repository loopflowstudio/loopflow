"""Loopflow CLI: Arrange LLMs to code in harmony."""

import platform
import shutil
import subprocess
import sys

import typer

from pathlib import Path

from loopflow.config import load_config
from loopflow.context import build_prompt, find_repo_root, gather_task
from loopflow.git import create_and_track_branch, open_pr
from loopflow.launcher import check_claude_available, launch_claude
from loopflow.pipeline import run_pipeline

app = typer.Typer(
    name="lf",
    help="Arrange LLMs to code in harmony.",
    no_args_is_help=True,
)


def _autocommit(repo_root: Path, task: str, arg: str | None) -> None:
    """Commit changes with the lf command as the message."""
    # Check if there are any changes to commit
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if not result.stdout.strip():
        return  # Nothing to commit

    # Build commit message from the command
    msg = f"lf {task}"
    if arg:
        msg += f" {arg}"

    subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", msg], cwd=repo_root, check=True)


@app.command()
def run(
    task: str = typer.Argument(help="Task name (e.g., 'review', 'implement')"),
    arg: str = typer.Argument(None, help="Input path for the task"),
    print_mode: bool = typer.Option(
        False, "-p", "-P", "--print", help="Run non-interactively"
    ),
    context: list[str] = typer.Option(
        None, "-c", "--context", help="Additional files for context"
    ),
    branch: str = typer.Option(
        None, "-b", "--branch", help="Create and track new branch"
    ),
):
    """Run a task with Claude."""
    repo_root = find_repo_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not check_claude_available():
        typer.echo("Error: 'claude' CLI not found. Run: lf install", err=True)
        raise typer.Exit(1)

    if branch:
        if not create_and_track_branch(repo_root, branch):
            typer.echo(f"Error: Could not create branch '{branch}'", err=True)
            raise typer.Exit(1)

    config = load_config(repo_root)
    skip_permissions = config.dangerously_skip_permissions if config else False

    # Merge config context with CLI context
    all_context = list(config.context) if config and config.context else []
    if context:
        all_context.extend(context)

    prompt = build_prompt(repo_root, task, arg=arg, context=all_context or None)
    exit_code, _ = launch_claude(
        prompt,
        print_mode=print_mode,
        stream=print_mode,
        skip_permissions=skip_permissions,
        cwd=repo_root,
    )

    if print_mode and exit_code == 0:
        _autocommit(repo_root, task, arg)

    raise typer.Exit(exit_code)


@app.command()
def version():
    """Show loopflow version."""
    from loopflow import __version__

    typer.echo(f"loopflow {__version__}")


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


@app.command()
def install():
    """Install loopflow dependencies (Node.js, Claude Code). macOS only."""
    if platform.system() != "Darwin":
        typer.echo("Error: lf install only supports macOS", err=True)
        typer.echo("Install Node.js and Claude Code manually.", err=True)
        raise typer.Exit(1)

    # Check/install Node.js
    if not shutil.which("npm"):
        typer.echo("Node.js not found.")
        if _install_node() and shutil.which("npm"):
            typer.echo("Node.js installed successfully.")
        else:
            typer.echo("Could not install Node.js.", err=True)
            typer.echo("Install Homebrew (https://brew.sh) then run: brew install node", err=True)
            raise typer.Exit(1)

    # Check/install Claude Code
    if check_claude_available():
        typer.echo("Claude Code is already installed.")
        raise typer.Exit(0)

    typer.echo("Installing Claude Code...")
    result = subprocess.run(
        ["npm", "install", "-g", "@anthropic-ai/claude-code"],
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        typer.echo(f"Error installing Claude Code:\n{result.stderr}", err=True)
        typer.echo("\nTry running manually: npm install -g @anthropic-ai/claude-code", err=True)
        raise typer.Exit(1)

    typer.echo("Claude Code installed successfully.")


@app.command()
def doctor():
    """Check loopflow dependencies."""
    all_ok = True

    if shutil.which("npm"):
        typer.echo("✓ npm")
    else:
        typer.echo("✗ npm - Install Node.js: https://nodejs.org")
        all_ok = False

    if check_claude_available():
        typer.echo("✓ claude")
    else:
        typer.echo("✗ claude - Run: lf install")
        all_ok = False

    # Optional: gh for PR creation
    if shutil.which("gh"):
        typer.echo("✓ gh (optional, for PR creation)")
    else:
        typer.echo("- gh (optional, for PR creation): brew install gh")

    raise typer.Exit(0 if all_ok else 1)


@app.command()
def pipeline(
    name: str = typer.Argument(help="Pipeline name from config.yaml"),
    arg: str = typer.Argument(None, help="Input for first task"),
    context: list[str] = typer.Option(
        None, "-c", "--context", help="Context files for all tasks"
    ),
    branch: str = typer.Option(
        None, "-b", "--branch", help="Create and track new branch"
    ),
    pr: bool = typer.Option(
        None, "--pr", help="Open PR when done"
    ),
):
    """Run a named pipeline."""
    repo_root = find_repo_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not check_claude_available():
        typer.echo("Error: 'claude' CLI not found. Run: lf install", err=True)
        raise typer.Exit(1)

    if branch:
        if not create_and_track_branch(repo_root, branch):
            typer.echo(f"Error: Could not create branch '{branch}'", err=True)
            raise typer.Exit(1)

    config = load_config(repo_root)
    if not config or name not in config.pipelines:
        typer.echo(f"Error: Pipeline '{name}' not found in .lf/config.yaml", err=True)
        raise typer.Exit(1)

    # Merge config context with CLI context
    all_context = list(config.context) if config.context else []
    if context:
        all_context.extend(context)

    # Flag overrides config
    push_enabled = config.push
    pr_enabled = pr if pr is not None else config.pr

    exit_code = run_pipeline(
        config.pipelines[name],
        repo_root,
        arg=arg,
        context=all_context or None,
        skip_permissions=config.dangerously_skip_permissions,
        push_enabled=push_enabled,
        pr_enabled=pr_enabled,
    )
    raise typer.Exit(exit_code)


@app.command()
def pr():
    """Create a GitHub PR for this branch."""
    repo_root = find_repo_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not shutil.which("gh"):
        typer.echo("Error: 'gh' CLI not found. Install with: brew install gh", err=True)
        raise typer.Exit(1)

    pr_url, error = open_pr(repo_root, draft=False)
    if pr_url:
        typer.echo(pr_url)
    else:
        typer.echo(f"Error: {error}", err=True)
        raise typer.Exit(1)


@app.command()
def land(
    message: str = typer.Option(None, "-m", "--message", help="Commit message"),
):
    """Land this branch: squash-merge to main and clean up."""
    repo_root = find_repo_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    # Get current branch
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    branch = result.stdout.strip()

    if not branch or branch == "main":
        typer.echo("Error: Already on main (or detached HEAD)", err=True)
        raise typer.Exit(1)

    # Check for uncommitted changes
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        typer.echo("Error: Uncommitted changes. Commit or stash first.", err=True)
        raise typer.Exit(1)

    # Get commit message from -m flag or .lf/COMMIT file
    commit_file = repo_root / ".lf" / "COMMIT"
    if message:
        commit_msg = message
    elif commit_file.exists():
        commit_msg = commit_file.read_text().strip()
        if not commit_msg:
            typer.echo("Error: .lf/COMMIT is empty", err=True)
            raise typer.Exit(1)
    else:
        typer.echo("Error: No commit message. Use -m or create .lf/COMMIT", err=True)
        raise typer.Exit(1)

    # Remove COMMIT file before merge so it doesn't end up in main
    if commit_file.exists():
        commit_file.unlink()
        subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)
        subprocess.run(
            ["git", "commit", "-m", "remove .lf/COMMIT before land"],
            cwd=repo_root,
            check=True,
        )

    # Land it
    subprocess.run(["git", "checkout", "main"], cwd=repo_root, check=True)
    subprocess.run(["git", "merge", "--squash", branch], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", commit_msg], cwd=repo_root, check=True)
    subprocess.run(["git", "branch", "-D", branch], cwd=repo_root, check=True)
    subprocess.run(["git", "push"], cwd=repo_root, check=True)

    typer.echo(f"Landed {branch} to main and pushed.")


def main():
    """Entry point that supports 'lf <task>' and 'lf <pipeline>' shorthand."""
    known_commands = {"run", "pipeline", "version", "install", "doctor", "pr", "land", "--help", "-h"}

    if len(sys.argv) > 1 and sys.argv[1] not in known_commands:
        name = sys.argv[1]
        repo_root = find_repo_root()
        config = load_config(repo_root) if repo_root else None

        has_pipeline = config and name in config.pipelines
        has_task = repo_root and gather_task(repo_root, name) is not None

        if has_pipeline and has_task:
            typer.echo(
                f"Error: '{name}' exists as both a pipeline and a task. "
                "Remove one to resolve the conflict.",
                err=True,
            )
            raise SystemExit(1)

        if has_pipeline:
            sys.argv.insert(1, "pipeline")
        else:
            sys.argv.insert(1, "run")

    app()


if __name__ == "__main__":
    main()

"""Loopflow CLI: Arrange LLMs to code in harmony."""

import shutil
import subprocess
import sys

import typer

from loopflow.context import build_prompt, find_repo_root
from loopflow.launcher import check_claude_available, launch_claude

app = typer.Typer(
    name="lf",
    help="Arrange LLMs to code in harmony.",
    no_args_is_help=True,
)


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
):
    """Run a task with Claude."""
    repo_root = find_repo_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not check_claude_available():
        typer.echo("Error: 'claude' CLI not found. Run: lf install", err=True)
        raise typer.Exit(1)

    prompt = build_prompt(
        repo_root, task, arg=arg, print_mode=print_mode, context=context
    )
    exit_code, _ = launch_claude(prompt, print_mode=print_mode, cwd=repo_root)
    raise typer.Exit(exit_code)


@app.command()
def version():
    """Show loopflow version."""
    from loopflow import __version__

    typer.echo(f"loopflow {__version__}")


@app.command()
def install():
    """Install loopflow dependencies (Claude Code)."""
    if not shutil.which("npm"):
        typer.echo("Error: npm not found. Install Node.js first: https://nodejs.org", err=True)
        raise typer.Exit(1)

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

    raise typer.Exit(0 if all_ok else 1)


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

    # Land it
    subprocess.run(["git", "checkout", "main"], cwd=repo_root, check=True)
    subprocess.run(["git", "merge", "--squash", branch], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", commit_msg], cwd=repo_root, check=True)
    subprocess.run(["git", "branch", "-D", branch], cwd=repo_root, check=True)

    # Clean up commit file if it was used
    if commit_file.exists():
        commit_file.unlink()

    typer.echo(f"Landed {branch} to main.")


def main():
    """Entry point that supports 'lf <task>' shorthand."""
    # If first arg looks like a task (not a known command, not a flag), inject 'run'
    known_commands = {"run", "version", "install", "doctor", "land", "--help", "-h"}

    if len(sys.argv) > 1 and sys.argv[1] not in known_commands:
        sys.argv.insert(1, "run")

    app()


if __name__ == "__main__":
    main()

"""Loopflow CLI: Arrange LLMs to code in harmony."""

import typer

from loopflow.context import build_prompt, find_repo_root
from loopflow.launcher import check_claude_available, launch_claude

app = typer.Typer(
    name="lf",
    help="Arrange LLMs to code in harmony.",
    no_args_is_help=True,
)


@app.command()
def claude(
    task: str = typer.Argument(help="Task name (e.g., 'review')"),
    print_mode: bool = typer.Option(
        False, "-p", "-P", "--print", help="Use Claude's print mode"
    ),
    role: str = typer.Option(
        None, "-r", "--role", help="Role file to use (default from settings.json)"
    ),
):
    """Launch a Claude Code session with context."""
    repo_root = find_repo_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not check_claude_available():
        typer.echo("Error: 'claude' CLI not found. Install Claude Code first.", err=True)
        raise typer.Exit(1)

    prompt = build_prompt(repo_root, task, role)
    exit_code = launch_claude(prompt, print_mode=print_mode, cwd=repo_root)
    raise typer.Exit(exit_code)


@app.command()
def version():
    """Show loopflow version."""
    from loopflow import __version__

    typer.echo(f"loopflow {__version__}")


if __name__ == "__main__":
    app()

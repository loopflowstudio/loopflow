"""Create new prompt files."""

import typer

from loopflow.lf.context import find_worktree_root

# Template for new prompt files
PROMPT_TEMPLATE = """\
---
produces: <results>
---
{name} step.

{{args}}
"""


def register_commands(app: typer.Typer) -> None:
    """Register add command on the app."""

    @app.command()
    def add(
        name: str = typer.Argument(help="Name for the new prompt (becomes filename and topic)"),
        force: bool = typer.Option(False, "-f", "-F", "--force", help="Overwrite if exists"),
    ):
        """Create a new prompt file at .claude/commands/<name>.md"""
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: must be in a git repository", err=True)
            raise typer.Exit(1)

        commands_dir = repo_root / ".claude" / "commands"
        target = commands_dir / f"{name}.md"

        if target.exists() and not force:
            typer.echo(f"Error: {target.relative_to(repo_root)} already exists", err=True)
            typer.echo("Use -f to overwrite", err=True)
            raise typer.Exit(1)

        commands_dir.mkdir(parents=True, exist_ok=True)
        target.write_text(PROMPT_TEMPLATE.format(name=name.capitalize()))

        typer.echo(f"Created {target.relative_to(repo_root)}")

"""Loopflow CLI: Arrange LLMs to code in harmony."""

import sys

import typer

from loopflow.config import ConfigError, load_config
from loopflow.context import find_worktree_root, gather_task
from loopflow.init_check import check_init_status

app = typer.Typer(
    name="lf",
    help="Arrange LLMs to code in harmony.",
    no_args_is_help=True,
)

# Import and register subcommands
from loopflow.cli import run as run_module

# Register top-level commands
app.command(context_settings={"allow_extra_args": True, "allow_interspersed_args": True})(run_module.run)
app.command()(run_module.inline)
app.command(name="pipeline")(run_module.pipeline)
app.command()(run_module.cp)


def main():
    """Entry point that supports 'lf <task>' and 'lf <pipeline>' shorthand."""
    known_commands = {
        "run",
        "pipeline",
        "inline",
        "cp",
        "--help",
        "-h",
    }

    try:
        if len(sys.argv) > 1:
            first_arg = sys.argv[1]

            # Inline prompt: lf : "prompt"
            if first_arg == ":":
                sys.argv.pop(1)
                sys.argv.insert(1, "inline")
            elif first_arg not in known_commands:
                # Handle colon suffix: "lf implement: add logout" -> "lf implement add logout"
                if first_arg.endswith(":"):
                    sys.argv[1] = first_arg[:-1]
                name = sys.argv[1]
                repo_root = find_worktree_root()
                config = load_config(repo_root) if repo_root else None

                has_pipeline = config and name in config.pipelines
                has_task = repo_root and gather_task(repo_root, name) is not None

                if has_pipeline and has_task:
                    typer.echo(f"Error: '{name}' exists as both a pipeline and a task", err=True)
                    typer.echo(f"  Pipeline: defined in .lf/config.yaml", err=True)
                    typer.echo(f"  Task: .claude/commands/{name}.md or .lf/{name}.*", err=True)
                    typer.echo(f"Remove one to resolve the conflict.", err=True)
                    raise SystemExit(1)

                if has_pipeline:
                    sys.argv.insert(1, "pipeline")
                elif has_task:
                    sys.argv.insert(1, "run")
                else:
                    # Check if repo is initialized
                    status = check_init_status(repo_root) if repo_root else None
                    if status and not status.has_commands and not status.has_lf_dir:
                        # Uninitialized repo - suggest init
                        typer.echo(f"No task named '{name}' found.", err=True)
                        typer.echo("", err=True)
                        typer.echo("This repo hasn't been set up for loopflow yet.", err=True)
                        typer.echo("Run: lfops init", err=True)
                    else:
                        # Initialized but task missing - suggest creating it
                        typer.echo(f"No task or pipeline named '{name}'", err=True)
                        typer.echo(f"Create: .claude/commands/{name}.md", err=True)
                    raise SystemExit(1)

        app()
    except ConfigError as e:
        typer.echo(f"Error: {e}", err=True)
        raise SystemExit(1)


if __name__ == "__main__":
    main()

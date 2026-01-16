"""Loopflow CLI: Arrange LLMs to code in harmony."""

import sys

import typer

from loopflow.config import ConfigError, load_config
from loopflow.context import find_worktree_root, gather_task, list_all_tasks, _get_builtin_task
from loopflow.init_check import check_init_status
from loopflow.lfd.pipelines import load_pipeline

app = typer.Typer(
    name="lf",
    help="Arrange LLMs to code in harmony.",
    no_args_is_help=False,
)

# Import and register subcommands
from loopflow.cli import run as run_module

# Register top-level commands
app.command(context_settings={"allow_extra_args": True, "allow_interspersed_args": True})(run_module.run)
app.command()(run_module.inline)
app.command(name="pipeline")(run_module.pipeline)
app.command()(run_module.cp)


def _list_tasks() -> None:
    """List available tasks and pipelines."""
    repo_root = find_worktree_root()
    config = load_config(repo_root) if repo_root else None

    user_tasks, builtin_only = list_all_tasks(repo_root)
    pipelines = list(config.pipelines.keys()) if config else []

    # Show pipelines
    if pipelines:
        typer.echo("Pipelines:")
        for name in sorted(pipelines):
            typer.echo(f"  {name}")
        typer.echo()

    # Show tasks
    if user_tasks or builtin_only:
        typer.echo("Tasks:")
        for name in user_tasks:
            typer.echo(f"  {name}")
        for name in builtin_only:
            typer.echo(f"  {name} (builtin)")
    else:
        typer.echo("No tasks found.")
        typer.echo("Run: lfops init")


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
        # Handle 'lf' with no arguments: list available tasks
        if len(sys.argv) == 1:
            _list_tasks()
            raise SystemExit(0)

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

                # Check for pipeline in config.yaml or .lf/pipelines/
                has_config_pipeline = config and name in config.pipelines
                has_file_pipeline = repo_root and load_pipeline(name, repo_root) is not None
                has_pipeline = has_config_pipeline or has_file_pipeline

                # gather_task now includes builtins
                has_task = gather_task(repo_root, name) is not None if repo_root else False
                # Also check builtin even without repo_root
                if not has_task and not repo_root:
                    has_task = _get_builtin_task(name) is not None

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
                    # Task not found
                    typer.echo(f"No task or pipeline named '{name}'", err=True)
                    typer.echo(f"Run 'lf' to see available tasks.", err=True)
                    raise SystemExit(1)

        app()
    except ConfigError as e:
        typer.echo(f"Error: {e}", err=True)
        raise SystemExit(1)


if __name__ == "__main__":
    main()

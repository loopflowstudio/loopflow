"""Loopflow CLI: Arrange LLMs to code in harmony."""

import sys

import typer

from loopflow.config import ConfigError, load_config
from loopflow.context import find_worktree_root, gather_task

app = typer.Typer(
    name="lf",
    help="Arrange LLMs to code in harmony.",
    no_args_is_help=True,
)

# Import and register subcommands
from loopflow.cli import run as run_module
from loopflow.cli import wt, pr, meta, maestro, status

app.add_typer(wt.app, name="wt")
app.add_typer(pr.app, name="pr")
app.add_typer(meta.app, name="meta")
app.add_typer(maestro.app, name="maestro")

# Register top-level commands
app.command(context_settings={"allow_extra_args": True, "allow_interspersed_args": True})(run_module.run)
app.command()(run_module.inline)
app.command(name="pipeline")(run_module.pipeline)
app.command()(status.status)


def main():
    """Entry point that supports 'lf <task>' and 'lf <pipeline>' shorthand."""
    known_commands = {"run", "pipeline", "inline", "wt", "pr", "meta", "maestro", "status", "--help", "-h"}

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
                    typer.echo(
                        f"Error: '{name}' exists as both a pipeline and a task. "
                        "Remove one to resolve the conflict.",
                        err=True,
                    )
                    raise SystemExit(1)

                if has_pipeline:
                    sys.argv.insert(1, "pipeline")
                elif has_task:
                    sys.argv.insert(1, "run")
                else:
                    typer.echo(f"Error: No task or pipeline named '{name}'", err=True)
                    typer.echo(f"Create .lf/{name}.lf or add '{name}' to pipelines in .lf/config.yaml", err=True)
                    raise SystemExit(1)

        app()
    except ConfigError as e:
        typer.echo(f"Error: {e}", err=True)
        raise SystemExit(1)


if __name__ == "__main__":
    main()

"""Loopflow CLI: Arrange LLMs to code in harmony."""

import sys
from typing import Optional

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


@app.command()
def capture(
    window: Optional[str] = typer.Argument(None, help="Window name to capture (fuzzy match)"),
    name: Optional[str] = typer.Option(None, "--name", "-n", help="Output filename (without extension)"),
    list_windows: bool = typer.Option(False, "--list", "-l", help="List visible windows"),
    open_file: bool = typer.Option(False, "--open", "-o", help="Open screenshot after capture"),
):
    """Capture a window screenshot to .design/screenshots/."""
    import subprocess

    from loopflow.capture import (
        list_windows as get_windows,
        find_window,
        capture_window,
        generate_screenshot_path,
        ScreenCaptureError,
    )

    if list_windows:
        windows = get_windows()
        if not windows:
            typer.echo("No windows found")
            raise typer.Exit(0)

        typer.echo("Visible windows:\n")
        for win in windows:
            title_part = f" - {win.title}" if win.title else ""
            typer.echo(f"  {win.app_name}{title_part}")
        raise typer.Exit(0)

    if not window:
        typer.echo("Error: Specify a window name or use --list to see available windows", err=True)
        raise typer.Exit(1)

    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    win = find_window(window)
    if not win:
        typer.echo(f"Error: No window matching '{window}'", err=True)
        typer.echo("Use --list to see available windows", err=True)
        raise typer.Exit(1)

    output_path = generate_screenshot_path(name, repo_root)
    try:
        capture_window(win, output_path)
    except ScreenCaptureError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    rel_path = output_path.relative_to(repo_root)
    typer.echo(f"Captured {win.app_name} → {rel_path}")

    if open_file:
        subprocess.run(["open", str(output_path)], check=False)


def main():
    """Entry point that supports 'lf <task>' and 'lf <pipeline>' shorthand."""
    known_commands = {
        "run",
        "pipeline",
        "inline",
        "cp",
        "capture",
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

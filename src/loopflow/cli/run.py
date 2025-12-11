"""Task execution commands."""

import typer

from loopflow.config import load_config
from loopflow.context import build_prompt, find_worktree_root
from loopflow.git import GitError, autocommit, create_worktree
from loopflow.launcher import check_claude_available, launch_claude
from loopflow.pipeline import run_pipeline


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
        None, "-b", "--branch", help="Create worktree and run task there"
    ),
):
    """Run a task with Claude."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not check_claude_available():
        typer.echo("Error: 'claude' CLI not found. Run: lf meta install", err=True)
        raise typer.Exit(1)

    if branch:
        try:
            worktree_path = create_worktree(repo_root, branch)
        except GitError as e:
            typer.echo(f"Error: {e}", err=True)
            raise typer.Exit(1)
        repo_root = worktree_path

    config = load_config(repo_root)
    skip_permissions = config.dangerously_skip_permissions if config else False

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
        autocommit(repo_root, task, arg)

    if branch:
        typer.echo(f"\nWorktree: {repo_root}")

    raise typer.Exit(exit_code)


def inline(
    prompt: str = typer.Argument(help="Inline prompt to run with Claude"),
    print_mode: bool = typer.Option(
        False, "-p", "-P", "--print", help="Run non-interactively"
    ),
    context: list[str] = typer.Option(
        None, "-c", "--context", help="Additional files for context"
    ),
):
    """Run an inline prompt with Claude."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not check_claude_available():
        typer.echo("Error: 'claude' CLI not found. Run: lf meta install", err=True)
        raise typer.Exit(1)

    config = load_config(repo_root)
    skip_permissions = config.dangerously_skip_permissions if config else False

    all_context = list(config.context) if config and config.context else []
    if context:
        all_context.extend(context)

    prompt_text = build_prompt(repo_root, task=None, inline=prompt, context=all_context or None)
    exit_code, _ = launch_claude(
        prompt_text,
        print_mode=print_mode,
        stream=print_mode,
        skip_permissions=skip_permissions,
        cwd=repo_root,
    )

    if print_mode and exit_code == 0:
        autocommit(repo_root, ":", prompt)

    raise typer.Exit(exit_code)


def pipeline(
    name: str = typer.Argument(help="Pipeline name from config.yaml"),
    arg: str = typer.Argument(None, help="Input for first task"),
    context: list[str] = typer.Option(
        None, "-c", "--context", help="Context files for all tasks"
    ),
    branch: str = typer.Option(
        None, "-b", "--branch", help="Create worktree and run pipeline there"
    ),
    pr: bool = typer.Option(
        None, "--pr", help="Open PR when done"
    ),
):
    """Run a named pipeline."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not check_claude_available():
        typer.echo("Error: 'claude' CLI not found. Run: lf meta install", err=True)
        raise typer.Exit(1)

    if branch:
        try:
            worktree_path = create_worktree(repo_root, branch)
        except GitError as e:
            typer.echo(f"Error: {e}", err=True)
            raise typer.Exit(1)
        repo_root = worktree_path

    config = load_config(repo_root)
    if not config or name not in config.pipelines:
        typer.echo(f"Error: Pipeline '{name}' not found in .lf/config.yaml", err=True)
        raise typer.Exit(1)

    all_context = list(config.context) if config.context else []
    if context:
        all_context.extend(context)

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

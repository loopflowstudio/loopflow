"""Task execution commands."""

import subprocess

import typer

from loopflow.config import load_config
from loopflow.context import find_worktree_root, gather_prompt_components, format_prompt
from loopflow.git import GitError, autocommit, create_worktree
from loopflow.launcher import check_claude_available, launch_claude
from loopflow.pipeline import run_pipeline
from loopflow.tokens import analyze_components


def _copy_to_clipboard(text: str) -> None:
    """Copy text to clipboard using pbcopy."""
    subprocess.run(["pbcopy"], input=text.encode(), check=True)


def run(
    task: str = typer.Argument(help="Task name (e.g., 'review', 'implement')"),
    print_mode: bool = typer.Option(
        False, "-p", "--print", help="Run non-interactively"
    ),
    context: list[str] = typer.Option(
        None, "-x", "--context", help="Additional files for context"
    ),
    worktree: str = typer.Option(
        None, "-w", "--worktree", help="Create worktree and run task there"
    ),
    copy: bool = typer.Option(
        False, "-c", "--copy", help="Copy prompt to clipboard and show token breakdown"
    ),
):
    """Run a task with Claude."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not copy and not check_claude_available():
        typer.echo("Error: 'claude' CLI not found. Run: lf meta install", err=True)
        raise typer.Exit(1)

    if worktree:
        try:
            worktree_path = create_worktree(repo_root, worktree)
        except GitError as e:
            typer.echo(f"Error: {e}", err=True)
            raise typer.Exit(1)
        repo_root = worktree_path

    config = load_config(repo_root)
    skip_permissions = config.dangerously_skip_permissions if config else False

    all_context = list(config.context) if config and config.context else []
    if context:
        all_context.extend(context)

    exclude = list(config.exclude) if config and config.exclude else None
    components = gather_prompt_components(repo_root, task, context=all_context or None, exclude=exclude)

    if copy:
        prompt = format_prompt(components)
        _copy_to_clipboard(prompt)
        tree = analyze_components(components)
        typer.echo(tree.format())
        typer.echo("\nCopied to clipboard.")
        raise typer.Exit(0)

    prompt = format_prompt(components)
    exit_code, _ = launch_claude(
        prompt,
        print_mode=print_mode,
        stream=print_mode,
        skip_permissions=skip_permissions,
        cwd=repo_root,
    )

    if print_mode and exit_code == 0:
        autocommit(repo_root, task)

    if worktree:
        typer.echo(f"\nWorktree: {repo_root}")

    raise typer.Exit(exit_code)


def inline(
    prompt: str = typer.Argument(help="Inline prompt to run with Claude"),
    print_mode: bool = typer.Option(
        False, "-p", "--print", help="Run non-interactively"
    ),
    context: list[str] = typer.Option(
        None, "-x", "--context", help="Additional files for context"
    ),
    copy: bool = typer.Option(
        False, "-c", "--copy", help="Copy prompt to clipboard and show token breakdown"
    ),
):
    """Run an inline prompt with Claude."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not copy and not check_claude_available():
        typer.echo("Error: 'claude' CLI not found. Run: lf meta install", err=True)
        raise typer.Exit(1)

    config = load_config(repo_root)
    skip_permissions = config.dangerously_skip_permissions if config else False

    all_context = list(config.context) if config and config.context else []
    if context:
        all_context.extend(context)

    exclude = list(config.exclude) if config and config.exclude else None
    components = gather_prompt_components(repo_root, task=None, inline=prompt, context=all_context or None, exclude=exclude)

    if copy:
        prompt_text = format_prompt(components)
        _copy_to_clipboard(prompt_text)
        tree = analyze_components(components)
        typer.echo(tree.format())
        typer.echo("\nCopied to clipboard.")
        raise typer.Exit(0)

    prompt_text = format_prompt(components)
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
    context: list[str] = typer.Option(
        None, "-x", "--context", help="Context files for all tasks"
    ),
    worktree: str = typer.Option(
        None, "-w", "--worktree", help="Create worktree and run pipeline there"
    ),
    pr: bool = typer.Option(
        None, "--pr", help="Open PR when done"
    ),
    copy: bool = typer.Option(
        False, "-c", "--copy", help="Copy first task prompt to clipboard and show token breakdown"
    ),
):
    """Run a named pipeline."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not copy and not check_claude_available():
        typer.echo("Error: 'claude' CLI not found. Run: lf meta install", err=True)
        raise typer.Exit(1)

    if worktree:
        try:
            worktree_path = create_worktree(repo_root, worktree)
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

    exclude = list(config.exclude) if config.exclude else None

    if copy:
        # Show tokens for first task in pipeline
        first_task = config.pipelines[name].tasks[0]
        components = gather_prompt_components(repo_root, first_task, context=all_context or None, exclude=exclude)
        prompt = format_prompt(components)
        _copy_to_clipboard(prompt)
        tree = analyze_components(components)
        typer.echo(f"Pipeline '{name}' first task: {first_task}\n")
        typer.echo(tree.format())
        typer.echo("\nCopied to clipboard.")
        raise typer.Exit(0)

    push_enabled = config.push
    pr_enabled = pr if pr is not None else config.pr

    exit_code = run_pipeline(
        config.pipelines[name],
        repo_root,
        context=all_context or None,
        exclude=exclude,
        skip_permissions=config.dangerously_skip_permissions,
        push_enabled=push_enabled,
        pr_enabled=pr_enabled,
    )
    raise typer.Exit(exit_code)

"""Task execution commands."""

import os
import subprocess
import uuid
from datetime import datetime
from pathlib import Path
from typing import Optional

import typer

from loopflow.config import load_config
from loopflow.context import find_worktree_root, gather_prompt_components, format_prompt
from loopflow.git import GitError, autocommit, create_worktree, find_main_repo
from loopflow.launcher import get_runner
from loopflow.maestro import Session, SessionStatus, connect_maestro
from loopflow.pipeline import run_pipeline
from loopflow.tokens import analyze_components


ModelType = Optional[str]


def _copy_to_clipboard(text: str) -> None:
    """Copy text to clipboard using pbcopy."""
    subprocess.run(["pbcopy"], input=text.encode(), check=True)


def run(
    ctx: typer.Context,
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
    model: ModelType = typer.Option(
        None, "-m", "--model", help="Model to use (claude, codex)"
    ),
    parallel: str = typer.Option(
        None, "--parallel", help="Run in parallel with multiple models (e.g., 'claude,codex')"
    ),
):
    """Run a task with an LLM model."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    # Handle parallel execution
    if parallel:
        models = [m.strip() for m in parallel.split(",")]
        for model_name in models:
            wt_name = f"{task}-{model_name}"
            cmd = ["lf", task, "-w", wt_name, "--model", model_name, "-p"]
            if ctx.args:
                cmd.extend(ctx.args)
            if context:
                for ctx_file in context:
                    cmd.extend(["-x", ctx_file])

            subprocess.Popen(
                cmd,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            typer.echo(f"Started {wt_name}")

        # Suggest maestro for notifications if not running
        if not connect_maestro():
            typer.echo("\nTip: Run 'lf maestro start' to get notifications when tasks complete")

        raise typer.Exit(0)

    config = load_config(repo_root)
    model_name = model or (config.model if config else "claude")

    try:
        runner = get_runner(model_name)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not copy and not runner.is_available():
        typer.echo(f"Error: '{model_name}' CLI not found", err=True)
        raise typer.Exit(1)

    if worktree:
        try:
            worktree_path = create_worktree(repo_root, worktree)
        except GitError as e:
            typer.echo(f"Error: {e}", err=True)
            raise typer.Exit(1)
        repo_root = worktree_path

    config = load_config(repo_root)
    skip_permissions = config.yolo if config else False

    all_context = list(config.context) if config and config.context else []
    if context:
        all_context.extend(context)

    exclude = list(config.exclude) if config and config.exclude else None
    args = ctx.args or None
    components = gather_prompt_components(repo_root, task, context=all_context or None, exclude=exclude, task_args=args)

    if copy:
        prompt = format_prompt(components)
        _copy_to_clipboard(prompt)
        tree = analyze_components(components)
        typer.echo(tree.format())
        typer.echo("\nCopied to clipboard.")
        raise typer.Exit(0)

    # Register with maestro if running
    main_repo = find_main_repo(repo_root) or repo_root
    session = Session(
        id=str(uuid.uuid4()),
        task=task,
        repo=main_repo,
        worktree=repo_root,
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
        pid=os.getpid() if print_mode else None,
    )
    maestro = connect_maestro()
    if maestro:
        maestro.register(session)

    try:
        prompt = format_prompt(components)
        result = runner.launch(
            prompt,
            print_mode=print_mode,
            stream=print_mode,
            skip_permissions=skip_permissions,
            cwd=repo_root,
        )

        if print_mode and result.exit_code == 0:
            autocommit(repo_root, task)

        if maestro:
            status = SessionStatus.COMPLETED if result.exit_code == 0 else SessionStatus.ERROR
            maestro.update(session.id, status)

        if worktree:
            typer.echo(f"\nWorktree: {repo_root}")

        raise typer.Exit(result.exit_code)
    finally:
        if maestro:
            maestro.unregister(session.id)


def inline(
    prompt: str = typer.Argument(help="Inline prompt to run"),
    print_mode: bool = typer.Option(
        False, "-p", "--print", help="Run non-interactively"
    ),
    context: list[str] = typer.Option(
        None, "-x", "--context", help="Additional files for context"
    ),
    copy: bool = typer.Option(
        False, "-c", "--copy", help="Copy prompt to clipboard and show token breakdown"
    ),
    model: ModelType = typer.Option(
        None, "-m", "--model", help="Model to use (claude, codex)"
    ),
):
    """Run an inline prompt with an LLM model."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    config = load_config(repo_root)
    model_name = model or (config.model if config else "claude")

    try:
        runner = get_runner(model_name)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not copy and not runner.is_available():
        typer.echo(f"Error: '{model_name}' CLI not found", err=True)
        raise typer.Exit(1)

    skip_permissions = config.yolo if config else False

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

    # Register with maestro if running
    main_repo = find_main_repo(repo_root) or repo_root
    session = Session(
        id=str(uuid.uuid4()),
        task="inline",
        repo=main_repo,
        worktree=repo_root,
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
        pid=os.getpid() if print_mode else None,
    )
    maestro = connect_maestro()
    if maestro:
        maestro.register(session)

    try:
        prompt_text = format_prompt(components)
        result = runner.launch(
            prompt_text,
            print_mode=print_mode,
            stream=print_mode,
            skip_permissions=skip_permissions,
            cwd=repo_root,
        )

        if print_mode and result.exit_code == 0:
            autocommit(repo_root, ":", prompt)

        if maestro:
            status = SessionStatus.COMPLETED if result.exit_code == 0 else SessionStatus.ERROR
            maestro.update(session.id, status)

        raise typer.Exit(result.exit_code)
    finally:
        if maestro:
            maestro.unregister(session.id)


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
    model: ModelType = typer.Option(
        None, "-m", "--model", help="Model to use (claude, codex)"
    ),
):
    """Run a named pipeline."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    config = load_config(repo_root)
    if not config or name not in config.pipelines:
        typer.echo(f"Error: Pipeline '{name}' not found in .lf/config.yaml", err=True)
        raise typer.Exit(1)

    model_name = model or config.model

    try:
        runner = get_runner(model_name)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not copy and not runner.is_available():
        typer.echo(f"Error: '{model_name}' CLI not found", err=True)
        raise typer.Exit(1)

    if worktree:
        try:
            worktree_path = create_worktree(repo_root, worktree)
        except GitError as e:
            typer.echo(f"Error: {e}", err=True)
            raise typer.Exit(1)
        repo_root = worktree_path

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
        skip_permissions=config.yolo,
        push_enabled=push_enabled,
        pr_enabled=pr_enabled,
        model=model_name,
    )
    raise typer.Exit(exit_code)

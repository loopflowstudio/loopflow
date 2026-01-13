"""Task execution commands."""

import os
import subprocess
import sys
import tempfile
import uuid
from datetime import datetime
from pathlib import Path
from typing import Optional

import typer

from loopflow.config import load_config, parse_model
from loopflow.context import find_worktree_root, gather_prompt_components, format_prompt
from loopflow.git import find_main_repo
from loopflow.launcher import (
    build_model_command,
    build_model_interactive_command,
    get_runner,
)
from loopflow.logging import get_model_env
from loopflow.maestro import Session, SessionStatus
from loopflow.maestro.db import DEFAULT_DB_PATH, save_session, update_session_status
from loopflow.pipeline import run_pipeline
from loopflow.tokens import analyze_components
from loopflow.worktrees import WorktreeError, create


ModelType = Optional[str]


def _write_prompt_file(prompt: str) -> str:
    """Write prompt to a temp file and return the path.

    The temp file is not deleted automatically; the caller should clean it up.
    Using a file avoids exposing the prompt in ps output.
    """
    fd, path = tempfile.mkstemp(prefix="lf-prompt-", suffix=".txt")
    os.write(fd, prompt.encode())
    os.close(fd)
    return path


def _copy_to_clipboard(text: str) -> None:
    """Copy text to clipboard using pbcopy."""
    subprocess.run(["pbcopy"], input=text.encode(), check=True)


def run(
    ctx: typer.Context,
    task: str = typer.Argument(help="Task name (e.g., 'review', 'implement')"),
    auto: bool = typer.Option(
        False, "-a", "--auto", help="Override to run in auto mode"
    ),
    interactive: bool = typer.Option(
        False, "-i", "--interactive", help="Override to run in interactive mode"
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
    paste: bool = typer.Option(
        False, "-v", "--paste", help="Include clipboard content in prompt"
    ),
    model: ModelType = typer.Option(
        None, "-m", "--model", help="Model to use (backend or backend:variant)"
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
            cmd = ["lf", task, "-w", wt_name, "--model", model_name, "-a"]
            if ctx.args:
                cmd.extend(ctx.args)
            if context:
                for ctx_file in context:
                    cmd.extend(["-x", ctx_file])

            subprocess.Popen(
                cmd,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                env=get_model_env(),
            )
            typer.echo(f"Started {wt_name}")

        raise typer.Exit(0)

    config = load_config(repo_root)

    # Determine run mode: default is auto unless task is in interactive list
    task_is_interactive_default = config and task in config.interactive

    # Flags override config defaults
    if interactive:
        is_interactive = True
    elif auto:
        is_interactive = False
    else:
        # No flag: use config or default (auto)
        is_interactive = task_is_interactive_default

    agent_model = model or (config.agent_model if config else "claude:opus")
    backend, model_variant = parse_model(agent_model)

    try:
        runner = get_runner(backend)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not copy and not runner.is_available():
        typer.echo(f"Error: '{backend}' CLI not found", err=True)
        raise typer.Exit(1)

    if worktree:
        try:
            worktree_path = create(repo_root, worktree)
        except WorktreeError as e:
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
    components = gather_prompt_components(
        repo_root,
        task,
        context=all_context or None,
        exclude=exclude,
        task_args=args,
        paste=paste,
        include_tests_for=config.include_tests_for if config else None,
        run_mode="interactive" if is_interactive else "auto",
    )

    if copy:
        prompt = format_prompt(components)
        _copy_to_clipboard(prompt)
        tree = analyze_components(components)
        typer.echo(tree.format())
        typer.echo("\nCopied to clipboard.")
        raise typer.Exit(0)

    db_path = DEFAULT_DB_PATH
    main_repo = find_main_repo(repo_root) or repo_root
    run_mode = "interactive" if is_interactive else "auto"
    session = Session(
        id=str(uuid.uuid4()),
        task=task,
        repo=main_repo,
        worktree=repo_root,
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
        pid=os.getpid() if not is_interactive else None,
        backend=backend,
        run_mode=run_mode,
    )
    save_session(db_path, session)

    prompt = format_prompt(components)
    prompt_file = _write_prompt_file(prompt)

    # Generate token summary for startup display
    tree = analyze_components(components)
    token_summary = tree.format()

    if is_interactive:
        command = build_model_interactive_command(
            backend,
            skip_permissions=skip_permissions,
            model_variant=model_variant,
            sandbox_root=repo_root.parent,
            workdir=repo_root,
        )
    else:
        command = build_model_command(
            backend,
            auto=True,
            stream=True,
            skip_permissions=skip_permissions,
            model_variant=model_variant,
            sandbox_root=repo_root.parent,
            workdir=repo_root,
        )

    # For interactive mode, run CLI directly to preserve terminal
    if is_interactive:
        typer.echo(f"\033[90m━━━ {task} ━━━\033[0m", err=True)
        for line in token_summary.split("\n"):
            typer.echo(f"\033[90m{line}\033[0m", err=True)
        typer.echo(err=True)

        # Read prompt and clean up file before exec
        prompt_content = Path(prompt_file).read_text()
        os.unlink(prompt_file)

        # Remove API keys so CLIs use subscriptions
        os.environ.pop("ANTHROPIC_API_KEY", None)
        os.environ.pop("OPENAI_API_KEY", None)

        # Run CLI directly (replaces current process)
        cmd_with_prompt = command + [prompt_content]
        os.chdir(repo_root)
        os.execvp(cmd_with_prompt[0], cmd_with_prompt)

    # For auto mode, use collector for logging
    collector_cmd = [
        sys.executable,
        "-m",
        "loopflow.maestro.collector",
        "--session-id",
        session.id,
        "--task",
        task,
        "--repo-root",
        str(repo_root),
        "--prompt-file",
        prompt_file,
        "--token-summary",
        token_summary,
        "--autocommit",
        "--foreground",
        "--",
        *command,
    ]

    process = subprocess.Popen(collector_cmd, cwd=repo_root, env=get_model_env())
    session.pid = process.pid
    save_session(db_path, session)
    result_code = process.wait()

    # Clean up prompt file
    os.unlink(prompt_file)

    status = SessionStatus.COMPLETED if result_code == 0 else SessionStatus.ERROR
    update_session_status(db_path, session.id, status)

    if worktree:
        typer.echo(f"\nWorktree: {repo_root}")

    raise typer.Exit(result_code)


def inline(
    prompt: str = typer.Argument(help="Inline prompt to run"),
    auto: bool = typer.Option(
        False, "-a", "--auto", help="Override to run in auto mode"
    ),
    interactive: bool = typer.Option(
        False, "-i", "--interactive", help="Override to run in interactive mode"
    ),
    context: list[str] = typer.Option(
        None, "-x", "--context", help="Additional files for context"
    ),
    copy: bool = typer.Option(
        False, "-c", "--copy", help="Copy prompt to clipboard and show token breakdown"
    ),
    paste: bool = typer.Option(
        False, "-v", "--paste", help="Include clipboard content in prompt"
    ),
    model: ModelType = typer.Option(
        None, "-m", "--model", help="Model to use (backend or backend:variant)"
    ),
):
    """Run an inline prompt with an LLM model."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    config = load_config(repo_root)

    # Determine run mode: default is auto for inline prompts
    if interactive:
        is_interactive = True
    elif auto:
        is_interactive = False
    else:
        # No flag: inline prompts default to auto
        is_interactive = False

    agent_model = model or (config.agent_model if config else "claude:opus")
    backend, model_variant = parse_model(agent_model)

    try:
        runner = get_runner(backend)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not copy and not runner.is_available():
        typer.echo(f"Error: '{backend}' CLI not found", err=True)
        raise typer.Exit(1)

    skip_permissions = config.yolo if config else False

    all_context = list(config.context) if config and config.context else []
    if context:
        all_context.extend(context)

    exclude = list(config.exclude) if config and config.exclude else None
    components = gather_prompt_components(
        repo_root,
        task=None,
        inline=prompt,
        context=all_context or None,
        exclude=exclude,
        paste=paste,
        include_tests_for=config.include_tests_for if config else None,
        run_mode="interactive" if is_interactive else "auto",
    )

    if copy:
        prompt_text = format_prompt(components)
        _copy_to_clipboard(prompt_text)
        tree = analyze_components(components)
        typer.echo(tree.format())
        typer.echo("\nCopied to clipboard.")
        raise typer.Exit(0)

    db_path = DEFAULT_DB_PATH
    main_repo = find_main_repo(repo_root) or repo_root
    run_mode = "interactive" if is_interactive else "auto"
    session = Session(
        id=str(uuid.uuid4()),
        task="inline",
        repo=main_repo,
        worktree=repo_root,
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
        pid=os.getpid() if not is_interactive else None,
        backend=backend,
        run_mode=run_mode,
    )
    save_session(db_path, session)

    prompt_text = format_prompt(components)
    prompt_file = _write_prompt_file(prompt_text)

    # Generate token summary for startup display
    tree = analyze_components(components)
    token_summary = tree.format()

    if is_interactive:
        command = build_model_interactive_command(
            backend,
            skip_permissions=skip_permissions,
            model_variant=model_variant,
            sandbox_root=repo_root.parent,
            workdir=repo_root,
        )
    else:
        command = build_model_command(
            backend,
            auto=True,
            stream=True,
            skip_permissions=skip_permissions,
            model_variant=model_variant,
            sandbox_root=repo_root.parent,
            workdir=repo_root,
        )

    # For interactive mode, run CLI directly to preserve terminal
    if is_interactive:
        typer.echo(f"\033[90m━━━ inline ━━━\033[0m", err=True)
        for line in token_summary.split("\n"):
            typer.echo(f"\033[90m{line}\033[0m", err=True)
        typer.echo(err=True)

        # Read prompt and clean up file before exec
        prompt_content = Path(prompt_file).read_text()
        os.unlink(prompt_file)

        # Remove API keys so CLIs use subscriptions
        os.environ.pop("ANTHROPIC_API_KEY", None)
        os.environ.pop("OPENAI_API_KEY", None)

        # Run CLI directly (replaces current process)
        cmd_with_prompt = command + [prompt_content]
        os.chdir(repo_root)
        os.execvp(cmd_with_prompt[0], cmd_with_prompt)

    # For auto mode, use collector for logging
    collector_cmd = [
        sys.executable,
        "-m",
        "loopflow.maestro.collector",
        "--session-id",
        session.id,
        "--task",
        "inline",
        "--repo-root",
        str(repo_root),
        "--prompt-file",
        prompt_file,
        "--token-summary",
        token_summary,
        "--autocommit",
        "--foreground",
        "--",
        *command,
    ]

    process = subprocess.Popen(collector_cmd, cwd=repo_root, env=get_model_env())
    session.pid = process.pid
    save_session(db_path, session)
    result_code = process.wait()

    # Clean up prompt file
    os.unlink(prompt_file)

    status = SessionStatus.COMPLETED if result_code == 0 else SessionStatus.ERROR
    update_session_status(db_path, session.id, status)

    raise typer.Exit(result_code)


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
        None, "-m", "--model", help="Model to use (backend or backend:variant)"
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

    agent_model = model or config.agent_model
    backend, model_variant = parse_model(agent_model)

    try:
        runner = get_runner(backend)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not copy and not runner.is_available():
        typer.echo(f"Error: '{backend}' CLI not found", err=True)
        raise typer.Exit(1)

    if worktree:
        try:
            worktree_path = create(repo_root, worktree)
        except WorktreeError as e:
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
        components = gather_prompt_components(
            repo_root,
            first_task,
            context=all_context or None,
            exclude=exclude,
            include_tests_for=config.include_tests_for if config else None,
        )
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
        include_tests_for=config.include_tests_for if config else None,
        skip_permissions=config.yolo,
        push_enabled=push_enabled,
        pr_enabled=pr_enabled,
        backend=backend,
        model_variant=model_variant,
    )
    raise typer.Exit(exit_code)

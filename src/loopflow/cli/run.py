"""Task execution commands."""

import os
import subprocess
import sys
import uuid
from datetime import datetime
from pathlib import Path
from typing import Optional

import typer

from loopflow.config import load_config, parse_model
from loopflow.context import find_worktree_root, gather_prompt_components, gather_task, format_prompt, PromptComponents
from loopflow.frontmatter import resolve_task_config, TaskConfig
from loopflow.voices import parse_voice_arg, VoiceNotFoundError
from loopflow.git import find_main_repo
from loopflow.launcher import (
    build_model_command,
    build_model_interactive_command,
    get_runner,
)
from loopflow.logging import get_model_env, write_prompt_file
from loopflow.lfd.client import log_session_start, log_session_end
from loopflow.lfd.models import Session, SessionStatus
from loopflow.pipeline import run_pipeline
from loopflow.tokens import analyze_components
from loopflow.worktrees import WorktreeError, create


ModelType = Optional[str]


def _copy_to_clipboard(text: str) -> None:
    """Copy text to clipboard using pbcopy."""
    subprocess.run(["pbcopy"], input=text.encode(), check=True)


def _execute_task(
    task_name: str,
    repo_root: Path,
    components: PromptComponents,
    is_interactive: bool,
    backend: str,
    model_variant: str | None,
    skip_permissions: bool,
) -> int:
    """Execute a task (run or inline) and return exit code.

    This shared helper handles session creation, command building, and execution
    for both named tasks and inline prompts.
    """
    prompt = format_prompt(components)
    prompt_file = write_prompt_file(prompt)

    tree = analyze_components(components)
    token_summary = tree.format()

    main_repo = find_main_repo(repo_root) or repo_root
    run_mode = "interactive" if is_interactive else "auto"
    session = Session(
        id=str(uuid.uuid4()),
        task=task_name,
        repo=str(main_repo),
        worktree=str(repo_root),
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
        pid=os.getpid() if not is_interactive else None,
        model=backend,
        run_mode=run_mode,
    )
    log_session_start(session)

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
        typer.echo(f"\033[90m━━━ {task_name} ━━━\033[0m", err=True)
        for line in token_summary.split("\n"):
            typer.echo(f"\033[90m{line}\033[0m", err=True)
        typer.echo(err=True)

        # Read prompt and clean up file before exec
        prompt_content = Path(prompt_file).read_text()
        try:
            os.unlink(prompt_file)
        except OSError:
            pass  # Best effort cleanup

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
        "loopflow.lfd.collector",
        "--session-id",
        session.id,
        "--task",
        task_name,
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

    # Don't strip API keys from collector env - it needs them for commit message generation.
    # The collector strips keys when spawning the actual agent CLI.
    process = subprocess.Popen(collector_cmd, cwd=repo_root)
    result_code = process.wait()

    # Clean up prompt file
    try:
        os.unlink(prompt_file)
    except OSError:
        pass  # Best effort cleanup

    status = SessionStatus.COMPLETED if result_code == 0 else SessionStatus.ERROR
    log_session_end(session.id, status)

    return result_code


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
    paste: Optional[bool] = typer.Option(
        None, "-v", "--paste/--no-paste", help="Include clipboard content in prompt"
    ),
    docs: Optional[bool] = typer.Option(
        None, "--docs/--no-docs", help="Include repo documentation (.md files)"
    ),
    diff: Optional[bool] = typer.Option(
        None, "--diff/--no-diff", help="Include raw branch diff against main"
    ),
    diff_files: Optional[bool] = typer.Option(
        None, "--diff-files/--no-diff-files", help="Include files touched by branch"
    ),
    model: ModelType = typer.Option(
        None, "-m", "--model", help="Model to use (backend or backend:variant)"
    ),
    voice: str = typer.Option(
        None, "--voice", help="Voice(s) to use (comma-separated, e.g., 'architect,concise')"
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

    if worktree:
        try:
            worktree_path = create(repo_root, worktree)
        except WorktreeError as e:
            typer.echo(f"Error: {e}", err=True)
            raise typer.Exit(1)
        repo_root = worktree_path
        config = load_config(repo_root)

    # Gather task file to get frontmatter config
    task_file = gather_task(repo_root, task)
    frontmatter = task_file.config if task_file else TaskConfig()

    # Parse voice arg
    cli_voices = parse_voice_arg(voice)

    # Resolve config: CLI > frontmatter > global > defaults
    resolved = resolve_task_config(
        task_name=task,
        global_config=config,
        frontmatter=frontmatter,
        cli_interactive=True if interactive else None,
        cli_auto=True if auto else None,
        cli_model=model,
        cli_context=list(context) if context else None,
        cli_voice=cli_voices or None,
    )

    is_interactive = resolved.interactive
    backend, model_variant = parse_model(resolved.model)

    try:
        runner = get_runner(backend)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not copy and not runner.is_available():
        typer.echo(f"Error: '{backend}' CLI not found", err=True)
        raise typer.Exit(1)

    skip_permissions = config.yolo if config else False

    # Build exclude list: resolved.exclude + resolved.include adjustment
    exclude_patterns = list(resolved.exclude)
    # If include has tests/**, don't exclude tests
    for pattern in resolved.include:
        if pattern in exclude_patterns:
            exclude_patterns.remove(pattern)

    # Resolve paste/docs/diff/diff_files flags (CLI overrides config)
    include_paste = paste if paste is not None else (config.paste if config else False)
    include_docs = docs if docs is not None else (config.docs if config else True)
    include_diff = diff if diff is not None else (config.diff if config else False)
    include_diff_files = diff_files if diff_files is not None else (config.diff_files if config else True)

    args = ctx.args or None
    try:
        components = gather_prompt_components(
            repo_root,
            task,
            context=resolved.context or None,
            exclude=exclude_patterns or None,
            task_args=args,
            paste=include_paste,
            run_mode="interactive" if is_interactive else "auto",
            include_loopflow_doc=config.include_loopflow_doc if config else True,
            voices=resolved.voice or None,
            include_diff=include_diff,
            include_diff_files=include_diff_files,
        )
    except VoiceNotFoundError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    # Apply docs flag
    if not include_docs:
        components.docs = []

    if copy:
        prompt = format_prompt(components)
        _copy_to_clipboard(prompt)
        tree = analyze_components(components)
        typer.echo(tree.format())
        typer.echo("\nCopied to clipboard.")
        raise typer.Exit(0)

    result_code = _execute_task(
        task,
        repo_root,
        components,
        is_interactive,
        backend,
        model_variant,
        skip_permissions,
    )

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
    paste: Optional[bool] = typer.Option(
        None, "-v", "--paste/--no-paste", help="Include clipboard content in prompt"
    ),
    docs: Optional[bool] = typer.Option(
        None, "--docs/--no-docs", help="Include repo documentation (.md files)"
    ),
    diff: Optional[bool] = typer.Option(
        None, "--diff/--no-diff", help="Include raw branch diff against main"
    ),
    diff_files: Optional[bool] = typer.Option(
        None, "--diff-files/--no-diff-files", help="Include files touched by branch"
    ),
    model: ModelType = typer.Option(
        None, "-m", "--model", help="Model to use (backend or backend:variant)"
    ),
    voice: str = typer.Option(
        None, "--voice", help="Voice(s) to use (comma-separated, e.g., 'architect,concise')"
    ),
):
    """Run an inline prompt with an LLM model."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    config = load_config(repo_root)

    # Parse voice arg
    cli_voices = parse_voice_arg(voice)

    # Resolve config for inline prompts (no frontmatter)
    resolved = resolve_task_config(
        task_name="inline",
        global_config=config,
        frontmatter=TaskConfig(),
        cli_interactive=True if interactive else None,
        cli_auto=True if auto else None,
        cli_model=model,
        cli_context=list(context) if context else None,
        cli_voice=cli_voices or None,
    )

    is_interactive = resolved.interactive
    backend, model_variant = parse_model(resolved.model)

    try:
        runner = get_runner(backend)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not copy and not runner.is_available():
        typer.echo(f"Error: '{backend}' CLI not found", err=True)
        raise typer.Exit(1)

    skip_permissions = config.yolo if config else False

    # Build exclude list from resolved config
    exclude_patterns = list(resolved.exclude)
    for pattern in resolved.include:
        if pattern in exclude_patterns:
            exclude_patterns.remove(pattern)

    # Resolve paste/docs/diff/diff_files flags (CLI overrides config)
    include_paste = paste if paste is not None else (config.paste if config else False)
    include_docs = docs if docs is not None else (config.docs if config else True)
    include_diff = diff if diff is not None else (config.diff if config else False)
    include_diff_files = diff_files if diff_files is not None else (config.diff_files if config else True)

    try:
        components = gather_prompt_components(
            repo_root,
            task=None,
            inline=prompt,
            context=resolved.context or None,
            exclude=exclude_patterns or None,
            paste=include_paste,
            run_mode="interactive" if is_interactive else "auto",
            include_loopflow_doc=config.include_loopflow_doc if config else True,
            voices=resolved.voice or None,
            include_diff=include_diff,
            include_diff_files=include_diff_files,
        )
    except VoiceNotFoundError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    # Apply docs flag
    if not include_docs:
        components.docs = []

    if copy:
        prompt_text = format_prompt(components)
        _copy_to_clipboard(prompt_text)
        tree = analyze_components(components)
        typer.echo(tree.format())
        typer.echo("\nCopied to clipboard.")
        raise typer.Exit(0)

    result_code = _execute_task(
        "inline",
        repo_root,
        components,
        is_interactive,
        backend,
        model_variant,
        skip_permissions,
    )

    raise typer.Exit(result_code)


def cp(
    paths: list[str] = typer.Argument(
        None, help="Files or directories to include (e.g., src tests)"
    ),
    exclude: list[str] = typer.Option(
        None, "-e", "--exclude", help="Patterns to exclude"
    ),
    paste: bool = typer.Option(
        False, "-v", "--paste", help="Include clipboard content"
    ),
    docs: Optional[bool] = typer.Option(
        None, "--docs/--no-docs", help="Include repo documentation (.md files)"
    ),
    diff: Optional[bool] = typer.Option(
        None, "--diff/--no-diff", help="Include raw branch diff"
    ),
    diff_files: Optional[bool] = typer.Option(
        None, "--diff-files/--no-diff-files", help="Include files touched by branch"
    ),
):
    """Copy file context to clipboard."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    config = load_config(repo_root)

    # Merge positional paths and config context
    all_context = list(paths or [])
    if config and config.context:
        all_context.extend(config.context)

    # Merge exclude patterns
    exclude_patterns = list(exclude or [])
    if config and config.exclude:
        exclude_patterns.extend(config.exclude)

    # Resolve flags (CLI overrides config)
    include_docs = docs if docs is not None else (config.docs if config else True)
    include_diff = diff if diff is not None else (config.diff if config else False)
    include_diff_files = diff_files if diff_files is not None else (config.diff_files if config else True)

    components = gather_prompt_components(
        repo_root,
        task=None,
        context=all_context or None,
        exclude=exclude_patterns or None,
        paste=paste,
        run_mode=None,
        include_loopflow_doc=config.include_loopflow_doc if config else True,
        include_diff=include_diff,
        include_diff_files=include_diff_files,
    )

    # Apply docs flag
    if not include_docs:
        components.docs = []

    prompt = format_prompt(components)
    _copy_to_clipboard(prompt)

    tree = analyze_components(components)
    typer.echo(tree.format())
    typer.echo("\nCopied to clipboard.")


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
            include_loopflow_doc=config.include_loopflow_doc,
            include_diff=config.diff,
            include_diff_files=config.diff_files,
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

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
from loopflow.lfd.pipelines import load_pipeline as load_pipeline_file, PipelineDef, PipelineStep, RaceConfig
from loopflow.pipeline import run_pipeline_def, _run_race_step
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
    chrome: bool = False,
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
            images=components.image_files,
            chrome=chrome,
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
            images=components.image_files,
            chrome=chrome,
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


def _launch_interactive_default(
    repo_root: Path,
    config,
    context: list[str] | None = None,
    model: str | None = None,
    voice: str | None = None,
    paste: bool | None = None,
    docs: bool | None = None,
    summaries: bool | None = None,
) -> None:
    """Launch interactive claude with docs context (no task)."""
    agent_model = model or (config.agent_model if config else "claude:opus")
    backend, model_variant = parse_model(agent_model)

    try:
        runner = get_runner(backend)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not runner.is_available():
        typer.echo(f"Error: '{backend}' CLI not found", err=True)
        raise typer.Exit(1)

    skip_permissions = config.yolo if config else False
    cli_voices = parse_voice_arg(voice)

    # Resolve flags
    include_paste = paste if paste is not None else (config.paste if config else False)
    include_docs = docs if docs is not None else (config.docs if config else True)
    include_summaries = summaries if summaries is not None else bool(config and config.summaries)

    try:
        components = gather_prompt_components(
            repo_root,
            task=None,
            inline=None,
            context=context,
            exclude=list(config.exclude) if config and config.exclude else None,
            paste=include_paste,
            run_mode="interactive",
            include_loopflow_doc=config.include_loopflow_doc if config else True,
            voices=cli_voices or (config.voice if config else None),
            include_diff=False,        # No diff without explicit task
            include_diff_files=False,  # No diff files without task
            include_summaries=include_summaries,
            config=config,
        )
    except VoiceNotFoundError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    # Apply docs flag
    if not include_docs:
        components.docs = []

    result_code = _execute_task(
        "chat",  # Task name for session tracking
        repo_root,
        components,
        is_interactive=True,
        backend=backend,
        model_variant=model_variant,
        skip_permissions=skip_permissions,
    )
    raise typer.Exit(result_code)


def run(
    ctx: typer.Context,
    task: Optional[str] = typer.Argument(None, help="Task name (e.g., 'review', 'implement')"),
    auto: bool = typer.Option(
        False, "-a", "-A", "--auto", help="Override to run in auto mode"
    ),
    interactive: bool = typer.Option(
        False, "-i", "-I", "--interactive", help="Override to run in interactive mode"
    ),
    context: list[str] = typer.Option(
        None, "-x", "-X", "--context", help="Additional files for context"
    ),
    worktree: str = typer.Option(
        None, "-w", "-W", "--worktree", help="Create worktree and run task there"
    ),
    copy: bool = typer.Option(
        False, "-c", "-C", "--copy", help="Copy prompt to clipboard and show token breakdown"
    ),
    paste: Optional[bool] = typer.Option(
        None, "-v", "-V", "--paste/--no-paste", help="Include clipboard content in prompt"
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
    summaries: Optional[bool] = typer.Option(
        None, "--summaries/--no-summaries", help="Include pre-generated codebase summaries"
    ),
    model: ModelType = typer.Option(
        None, "-m", "-M", "--model", help="Model to use (backend or backend:variant)"
    ),
    voice: str = typer.Option(
        None, "--voice", help="Voice(s) to use (comma-separated, e.g., 'architect,concise')"
    ),
    parallel: str = typer.Option(
        None, "--parallel", help="Run in parallel with multiple models, keep worktrees (e.g., 'claude,codex')"
    ),
    race: str = typer.Option(
        None, "--race", help="Race multiple models, auto-judge winner (e.g., 'claude,codex,gemini')"
    ),
    with_prompt: list[str] = typer.Option(
        None, "-p", "-P", "--prompt", help="Append additional prompt files (e.g., -p nux)"
    ),
    chrome: Optional[bool] = typer.Option(
        None, "--chrome/--no-chrome", help="Enable Chrome integration for Claude Code (browser automation)"
    ),
):
    """Run a task with an LLM model."""
    repo_root = find_worktree_root()

    # Some features require a git repo
    if not repo_root:
        if worktree:
            typer.echo("Error: --worktree requires a git repository", err=True)
            raise typer.Exit(1)
        if parallel:
            typer.echo("Error: --parallel requires a git repository", err=True)
            raise typer.Exit(1)
        if race:
            typer.echo("Error: --race requires a git repository", err=True)
            raise typer.Exit(1)
        # Use cwd as fallback for non-git usage
        repo_root = Path.cwd()

    # Handle race execution
    if race:
        models = [m.strip() for m in race.split(",")]
        config = load_config(repo_root)
        main_repo = find_main_repo(repo_root) or repo_root
        skip_permissions = config.yolo if config else False
        exclude = list(config.exclude) if config and config.exclude else None

        race_config = RaceConfig(models=models)
        result_code = _run_race_step(
            task,
            race_config,
            repo_root,
            main_repo,
            exclude,
            skip_permissions,
            list(context) if context else None,
            1,  # step_num
            1,  # total_steps
        )
        raise typer.Exit(result_code)

    # Handle parallel execution (creates persistent worktrees)
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

    # Handle no task: launch interactive claude with docs context
    if task is None:
        return _launch_interactive_default(
            repo_root,
            config,
            context=list(context) if context else None,
            model=model,
            voice=voice,
            paste=paste,
            docs=docs,
            summaries=summaries,
        )

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

    # Resolve paste/docs/diff/diff_files/summaries flags (CLI overrides config)
    include_paste = paste if paste is not None else (config.paste if config else False)
    include_docs = docs if docs is not None else (config.docs if config else True)
    include_diff = diff if diff is not None else (config.diff if config else False)
    include_diff_files = diff_files if diff_files is not None else (config.diff_files if config else True)
    include_summaries = summaries if summaries is not None else bool(config and config.summaries)

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
            include_summaries=include_summaries,
            config=config,
            with_prompts=list(with_prompt) if with_prompt else None,
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

    # Resolve chrome: CLI > frontmatter > config > default
    if chrome is not None:
        chrome_enabled = chrome
    elif frontmatter.chrome is not None:
        chrome_enabled = frontmatter.chrome
    elif config:
        chrome_enabled = config.chrome
    else:
        chrome_enabled = False

    result_code = _execute_task(
        task,
        repo_root,
        components,
        is_interactive,
        backend,
        model_variant,
        skip_permissions,
        chrome=chrome_enabled,
    )

    if worktree:
        typer.echo(f"\nWorktree: {repo_root}")

    raise typer.Exit(result_code)


def inline(
    prompt: str = typer.Argument(help="Inline prompt to run"),
    auto: bool = typer.Option(
        False, "-a", "-A", "--auto", help="Override to run in auto mode"
    ),
    interactive: bool = typer.Option(
        False, "-i", "-I", "--interactive", help="Override to run in interactive mode"
    ),
    context: list[str] = typer.Option(
        None, "-x", "-X", "--context", help="Additional files for context"
    ),
    copy: bool = typer.Option(
        False, "-c", "-C", "--copy", help="Copy prompt to clipboard and show token breakdown"
    ),
    paste: Optional[bool] = typer.Option(
        None, "-v", "-V", "--paste/--no-paste", help="Include clipboard content in prompt"
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
    summaries: Optional[bool] = typer.Option(
        None, "--summaries/--no-summaries", help="Include pre-generated codebase summaries"
    ),
    model: ModelType = typer.Option(
        None, "-m", "-M", "--model", help="Model to use (backend or backend:variant)"
    ),
    voice: str = typer.Option(
        None, "--voice", help="Voice(s) to use (comma-separated, e.g., 'architect,concise')"
    ),
    chrome: Optional[bool] = typer.Option(
        None, "--chrome/--no-chrome", help="Enable Chrome integration for Claude Code (browser automation)"
    ),
):
    """Run an inline prompt with an LLM model."""
    repo_root = find_worktree_root()
    if not repo_root:
        # Use cwd as fallback for non-git usage
        repo_root = Path.cwd()

    config = load_config(repo_root) if (repo_root / ".lf" / "config.yaml").exists() else None

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

    # Resolve paste/docs/diff/diff_files/summaries flags (CLI overrides config)
    include_paste = paste if paste is not None else (config.paste if config else False)
    include_docs = docs if docs is not None else (config.docs if config else True)
    include_diff = diff if diff is not None else (config.diff if config else False)
    include_diff_files = diff_files if diff_files is not None else (config.diff_files if config else True)
    include_summaries = summaries if summaries is not None else bool(config and config.summaries)

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
            include_summaries=include_summaries,
            config=config,
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

    # Resolve chrome: CLI > config > default
    if chrome is not None:
        chrome_enabled = chrome
    elif config:
        chrome_enabled = config.chrome
    else:
        chrome_enabled = False

    result_code = _execute_task(
        "inline",
        repo_root,
        components,
        is_interactive,
        backend,
        model_variant,
        skip_permissions,
        chrome=chrome_enabled,
    )

    raise typer.Exit(result_code)


def cp(
    paths: list[str] = typer.Argument(
        None, help="Files or directories to include (e.g., src tests)"
    ),
    exclude: list[str] = typer.Option(
        None, "-e", "-E", "--exclude", help="Patterns to exclude"
    ),
    paste: bool = typer.Option(
        False, "-v", "-V", "--paste", help="Include clipboard content"
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
    summaries: Optional[bool] = typer.Option(
        None, "--summaries/--no-summaries", help="Include pre-generated codebase summaries"
    ),
):
    """Copy file context to clipboard."""
    repo_root = find_worktree_root()
    if not repo_root:
        # Use cwd as fallback for non-git usage
        repo_root = Path.cwd()

    config = load_config(repo_root) if (repo_root / ".lf" / "config.yaml").exists() else None

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
    include_summaries = summaries if summaries is not None else bool(config and config.summaries)

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
        include_summaries=include_summaries,
        config=config,
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
    name: str = typer.Argument(help="Pipeline name from config.yaml or .lf/pipelines/"),
    context: list[str] = typer.Option(
        None, "-x", "-X", "--context", help="Context files for all tasks"
    ),
    worktree: str = typer.Option(
        None, "-w", "-W", "--worktree", help="Create worktree and run pipeline there"
    ),
    pr: bool = typer.Option(
        None, "--pr", help="Open PR when done"
    ),
    copy: bool = typer.Option(
        False, "-c", "-C", "--copy", help="Copy first task prompt to clipboard and show token breakdown"
    ),
    model: ModelType = typer.Option(
        None, "-m", "-M", "--model", help="Model to use (backend or backend:variant)"
    ),
):
    """Run a named pipeline."""
    repo_root = find_worktree_root()

    # Worktree creation still requires git
    if not repo_root and worktree:
        typer.echo("Error: --worktree requires a git repository", err=True)
        raise typer.Exit(1)

    # Use cwd as fallback for non-git usage
    if not repo_root:
        repo_root = Path.cwd()

    config = load_config(repo_root)

    # Check for pipeline in .lf/pipelines/ first, then config.yaml
    pipeline_def = load_pipeline_file(name, repo_root)
    config_pipeline = config.pipelines.get(name) if config else None

    if not pipeline_def and not config_pipeline:
        typer.echo(f"Error: Pipeline '{name}' not found in .lf/pipelines/ or .lf/config.yaml", err=True)
        raise typer.Exit(1)

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

    all_context = list(config.context) if config and config.context else []
    if context:
        all_context.extend(context)

    exclude = list(config.exclude) if config and config.exclude else None

    if copy:
        # Show tokens for first task in pipeline
        if pipeline_def:
            first_task = pipeline_def.steps[0].task if pipeline_def.steps else None
        else:
            first_task = config_pipeline.tasks[0] if config_pipeline.tasks else None

        if not first_task:
            typer.echo("Error: Pipeline has no tasks", err=True)
            raise typer.Exit(1)

        components = gather_prompt_components(
            repo_root,
            first_task,
            context=all_context or None,
            exclude=exclude,
            include_tests_for=config.include_tests_for if config else None,
            include_loopflow_doc=config.include_loopflow_doc if config else True,
            include_diff=config.diff if config else False,
            include_diff_files=config.diff_files if config else True,
            config=config,
        )
        prompt = format_prompt(components)
        _copy_to_clipboard(prompt)
        tree = analyze_components(components)
        typer.echo(f"Pipeline '{name}' first task: {first_task}\n")
        typer.echo(tree.format())
        typer.echo("\nCopied to clipboard.")
        raise typer.Exit(0)

    push_enabled = config.push if config else False
    pr_enabled = pr if pr is not None else (config.pr if config else False)
    skip_permissions = config.yolo if config else False
    chrome_enabled = config.chrome if config else False

    # Convert config.yaml pipeline to PipelineDef if needed
    if not pipeline_def:
        # PipelineConfig.push/pr override global settings
        push_enabled = config_pipeline.push if config_pipeline.push is not None else push_enabled
        pr_enabled = config_pipeline.pr if config_pipeline.pr is not None else pr_enabled
        pipeline_def = PipelineDef(
            name=name,
            steps=[PipelineStep(task=t) for t in config_pipeline.tasks],
        )

    exit_code = run_pipeline_def(
        pipeline_def,
        repo_root,
        context=all_context or None,
        exclude=exclude,
        skip_permissions=skip_permissions,
        push_enabled=push_enabled,
        pr_enabled=pr_enabled,
        backend=backend,
        model_variant=model_variant,
        chrome=chrome_enabled,
    )
    raise typer.Exit(exit_code)

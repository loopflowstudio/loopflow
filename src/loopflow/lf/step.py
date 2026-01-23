"""Step execution commands."""

import os
import subprocess
import sys
import uuid
from datetime import datetime
from pathlib import Path
from typing import Optional

import typer

from loopflow.lf.config import load_config, parse_model
from loopflow.lf.context import (
    ContextConfig,
    DiffMode,
    FilesetConfig,
    PromptComponents,
    find_worktree_root,
    format_prompt,
    gather_prompt_components,
    gather_step,
)
from loopflow.lf.flow import run_flow_def
from loopflow.lf.flows import load_flow
from loopflow.lf.frontmatter import StepConfig, resolve_step_config
from loopflow.lf.git import find_main_repo
from loopflow.lf.launcher import (
    build_model_command,
    build_model_interactive_command,
    get_runner,
)
from loopflow.lf.logging import write_prompt_file
from loopflow.lf.output import (
    copy_to_clipboard,
    trim_components_if_needed,
    warn_if_context_too_large,
)
from loopflow.lf.tokens import analyze_components
from loopflow.lf.voices import VoiceNotFoundError, parse_voice_arg
from loopflow.lf.worktrees import WorktreeError, create
from loopflow.lfd.models import StepRun, StepRunStatus
from loopflow.lfd.step_run import log_step_run_end, log_step_run_start

ModelType = Optional[str]

# Web client URLs for --web flag
WEB_CLIENTS = {
    "claude": "https://claude.ai/new",
    "codex": "https://chatgpt.com",
    "gemini": "https://aistudio.google.com/prompts/new_chat",
}


def _open_web_client(backend: str) -> None:
    """Open the web client for the given backend."""
    url = WEB_CLIENTS.get(backend, WEB_CLIENTS["claude"])
    subprocess.run(["open", url], check=True)


def _execute_step(
    step_name: str,
    repo_root: Path,
    components: PromptComponents,
    is_interactive: bool,
    backend: str,
    model_variant: str | None,
    skip_permissions: bool,
    chrome: bool = False,
) -> int:
    """Execute a step (run or inline) and return exit code.

    This shared helper handles session creation, command building, and execution
    for both named steps and inline prompts.
    """
    prompt = format_prompt(components)
    prompt_file = write_prompt_file(prompt)

    tree = analyze_components(components)
    token_summary = tree.format()

    warn_if_context_too_large(tree)

    main_repo = find_main_repo(repo_root) or repo_root
    run_mode = "interactive" if is_interactive else "auto"
    step_run = StepRun(
        id=str(uuid.uuid4()),
        step=step_name,
        repo=str(main_repo),
        worktree=str(repo_root),
        status=StepRunStatus.RUNNING,
        started_at=datetime.now(),
        pid=os.getpid() if not is_interactive else None,
        model=backend,
        run_mode=run_mode,
    )
    log_step_run_start(step_run)

    if is_interactive:
        command = build_model_interactive_command(
            backend,
            skip_permissions=skip_permissions,
            yolo=skip_permissions,
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
            yolo=skip_permissions,
            model_variant=model_variant,
            sandbox_root=repo_root.parent,
            workdir=repo_root,
            images=components.image_files,
            chrome=chrome,
        )

    # For interactive mode, run CLI directly to preserve terminal
    if is_interactive:
        typer.echo(f"\033[90m━━━ {step_name} ━━━\033[0m", err=True)
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
        "loopflow.lfd.execution.collector",
        "--step-run-id",
        step_run.id,
        "--step",
        step_name,
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

    status = StepRunStatus.COMPLETED if result_code == 0 else StepRunStatus.FAILED
    log_step_run_end(step_run.id, status)

    return result_code


def _launch_interactive_default(
    repo_root: Path,
    config,
    context: list[str] | None = None,
    model: str | None = None,
    voice: str | None = None,
    clipboard: bool | None = None,
    docs: bool | None = None,
) -> None:
    """Launch interactive claude with docs context (no step)."""
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
    include_clipboard = clipboard if clipboard is not None else (config.paste if config else False)
    include_docs = docs if docs is not None else (config.lfdocs if config else True)

    try:
        components = gather_prompt_components(
            repo_root,
            step=None,
            inline=None,
            run_mode="interactive",
            voices=cli_voices or (config.voice if config else None),
            context_config=ContextConfig.for_interactive(
                paths=list(context) if context else [],
                exclude=list(config.exclude) if config and config.exclude else [],
                lfdocs=config.include_loopflow_doc if config else True,
                clipboard=include_clipboard,
            ),
            config=config,
        )
    except VoiceNotFoundError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    # Apply docs flag
    if not include_docs:
        components.docs = []

    components = trim_components_if_needed(components)

    result_code = _execute_step(
        "chat",  # Step name for session tracking
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
    step: Optional[str] = typer.Argument(None, help="Step name (e.g., 'review', 'implement')"),
    auto: bool = typer.Option(False, "-a", "-A", "--auto", help="Override to run in auto mode"),
    interactive: bool = typer.Option(
        False, "-i", "-I", "--interactive", help="Override to run in interactive mode"
    ),
    path: list[str] = typer.Option(None, "-p", "-P", "--path", help="Additional files to include"),
    worktree: str = typer.Option(
        None, "-w", "-W", "--worktree", help="Create worktree and run step there"
    ),
    web: bool = typer.Option(
        False, "--web", help="Copy to clipboard and open web client (claude.ai, chatgpt.com, etc.)"
    ),
    clipboard: Optional[bool] = typer.Option(
        None, "-c", "-C", "--clipboard/--no-clipboard", help="Include clipboard content in prompt"
    ),
    docs: Optional[bool] = typer.Option(
        None, "--lfdocs/--no-lfdocs", help="Include roadmap/, scratch/, and root .md files"
    ),
    diff_mode: Optional[str] = typer.Option(
        None, "--diff-mode", help="How to include branch changes: files, diff, or none"
    ),
    model: ModelType = typer.Option(
        None, "-m", "-M", "--model", help="Model to use (backend or backend:variant)"
    ),
    voice: str = typer.Option(None, "-v", "-V", "--voice", help="Voices to use (comma-separated)"),
    chrome: Optional[bool] = typer.Option(
        None, "--chrome/--no-chrome", help="Enable Chrome browser automation"
    ),
):
    """Run a step with an LLM model."""
    repo_root = find_worktree_root()

    # Some features require a git repo
    if not repo_root:
        if worktree:
            typer.echo("Error: --worktree requires a git repository", err=True)
            raise typer.Exit(1)
        # Use cwd as fallback for non-git usage
        repo_root = Path.cwd()

    config = load_config(repo_root)

    if worktree:
        try:
            worktree_path = create(repo_root, worktree)
        except WorktreeError as e:
            typer.echo(f"Error: {e}", err=True)
            raise typer.Exit(1)
        repo_root = worktree_path
        config = load_config(repo_root)

    # Handle no step: launch interactive claude with docs context
    if step is None:
        return _launch_interactive_default(
            repo_root,
            config,
            context=list(path) if path else None,
            model=model,
            voice=voice,
            clipboard=clipboard,
            docs=docs,
        )

    # Gather step file to get frontmatter config
    step_file = gather_step(repo_root, step, config)
    frontmatter = step_file.config if step_file else StepConfig()

    # Parse voice arg
    cli_voices = parse_voice_arg(voice)

    # Resolve config: CLI > frontmatter > global > defaults
    resolved = resolve_step_config(
        step_name=step,
        global_config=config,
        frontmatter=frontmatter,
        cli_interactive=True if interactive else None,
        cli_auto=True if auto else None,
        cli_model=model,
        cli_context=list(path) if path else None,
        cli_voice=cli_voices or None,
    )

    is_interactive = resolved.interactive
    backend, model_variant = parse_model(resolved.model)

    try:
        runner = get_runner(backend)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not web and not runner.is_available():
        typer.echo(f"Error: '{backend}' CLI not found", err=True)
        raise typer.Exit(1)

    skip_permissions = config.yolo if config else False

    # Build exclude list: resolved.exclude + resolved.include adjustment
    exclude_patterns = list(resolved.exclude)
    # If include has tests/**, don't exclude tests
    for pattern in resolved.include:
        if pattern in exclude_patterns:
            exclude_patterns.remove(pattern)

    # Resolve clipboard/docs flags (CLI > config > default)
    include_clipboard = clipboard if clipboard is not None else (config.paste if config else False)
    include_docs = docs if docs is not None else (config.lfdocs if config else True)

    # Resolve diff_mode: CLI > frontmatter > config > default
    resolved_diff_mode = DiffMode.FILES  # default
    if diff_mode is not None:
        resolved_diff_mode = DiffMode(diff_mode)
    elif frontmatter.diff_files is False:
        resolved_diff_mode = DiffMode.NONE
    elif config and not config.diff_files:
        resolved_diff_mode = DiffMode.NONE
    elif config and config.diff:
        resolved_diff_mode = DiffMode.DIFF

    args = ctx.args or None
    try:
        components = gather_prompt_components(
            repo_root,
            step,
            step_args=args,
            run_mode="interactive" if is_interactive else "auto",
            voices=resolved.voice or None,
            context_config=ContextConfig(
                diff_mode=resolved_diff_mode,
                files=FilesetConfig(
                    paths=list(resolved.context) if resolved.context else [],
                    exclude=list(exclude_patterns) if exclude_patterns else [],
                ),
                area=resolved.area,
                lfdocs=config.include_loopflow_doc if config else True,
                clipboard=include_clipboard,
            ),
            config=config,
        )
    except VoiceNotFoundError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    # Apply docs flag
    if not include_docs:
        components.docs = []

    components = trim_components_if_needed(components)

    if web:
        prompt = format_prompt(components)
        copy_to_clipboard(prompt)
        tree = analyze_components(components)
        typer.echo(tree.format())
        warn_if_context_too_large(tree)
        typer.echo("\nCopied to clipboard.")
        _open_web_client(backend)
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

    result_code = _execute_step(
        step,
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
    auto: bool = typer.Option(False, "-a", "-A", "--auto", help="Override to run in auto mode"),
    interactive: bool = typer.Option(
        False, "-i", "-I", "--interactive", help="Override to run in interactive mode"
    ),
    path: list[str] = typer.Option(None, "-p", "-P", "--path", help="Additional files to include"),
    web: bool = typer.Option(
        False, "--web", help="Copy to clipboard and open web client (claude.ai, chatgpt.com, etc.)"
    ),
    clipboard: Optional[bool] = typer.Option(
        None, "-c", "-C", "--clipboard/--no-clipboard", help="Include clipboard content in prompt"
    ),
    docs: Optional[bool] = typer.Option(
        None, "--lfdocs/--no-lfdocs", help="Include roadmap/, scratch/, and root .md files"
    ),
    diff_mode: Optional[str] = typer.Option(
        None, "--diff-mode", help="How to include branch changes: files, diff, or none"
    ),
    model: ModelType = typer.Option(
        None, "-m", "-M", "--model", help="Model to use (backend or backend:variant)"
    ),
    voice: str = typer.Option(None, "-v", "-V", "--voice", help="Voices to use (comma-separated)"),
    chrome: Optional[bool] = typer.Option(
        None, "--chrome/--no-chrome", help="Enable Chrome browser automation"
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
    resolved = resolve_step_config(
        step_name="inline",
        global_config=config,
        frontmatter=StepConfig(),
        cli_interactive=True if interactive else None,
        cli_auto=True if auto else None,
        cli_model=model,
        cli_context=list(path) if path else None,
        cli_voice=cli_voices or None,
    )

    is_interactive = resolved.interactive
    backend, model_variant = parse_model(resolved.model)

    try:
        runner = get_runner(backend)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not web and not runner.is_available():
        typer.echo(f"Error: '{backend}' CLI not found", err=True)
        raise typer.Exit(1)

    skip_permissions = config.yolo if config else False

    # Build exclude list from resolved config
    exclude_patterns = list(resolved.exclude)
    for pattern in resolved.include:
        if pattern in exclude_patterns:
            exclude_patterns.remove(pattern)

    # Resolve clipboard/docs flags (CLI overrides config)
    include_clipboard = clipboard if clipboard is not None else (config.paste if config else False)
    include_docs = docs if docs is not None else (config.lfdocs if config else True)

    # Resolve diff_mode: CLI > config > default
    resolved_diff_mode = DiffMode.FILES  # default
    if diff_mode is not None:
        resolved_diff_mode = DiffMode(diff_mode)
    elif config and not config.diff_files:
        resolved_diff_mode = DiffMode.NONE
    elif config and config.diff:
        resolved_diff_mode = DiffMode.DIFF

    try:
        components = gather_prompt_components(
            repo_root,
            step=None,
            inline=prompt,
            run_mode="interactive" if is_interactive else "auto",
            voices=resolved.voice or None,
            context_config=ContextConfig(
                diff_mode=resolved_diff_mode,
                files=FilesetConfig(
                    paths=list(resolved.context) if resolved.context else [],
                    exclude=list(exclude_patterns) if exclude_patterns else [],
                ),
                area=resolved.area,
                lfdocs=config.include_loopflow_doc if config else True,
                clipboard=include_clipboard,
            ),
            config=config,
        )
    except VoiceNotFoundError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    # Apply docs flag
    if not include_docs:
        components.docs = []

    components = trim_components_if_needed(components)

    if web:
        prompt_text = format_prompt(components)
        copy_to_clipboard(prompt_text)
        tree = analyze_components(components)
        typer.echo(tree.format())
        warn_if_context_too_large(tree)
        typer.echo("\nCopied to clipboard.")
        _open_web_client(backend)
        raise typer.Exit(0)

    # Resolve chrome: CLI > config > default
    if chrome is not None:
        chrome_enabled = chrome
    elif config:
        chrome_enabled = config.chrome
    else:
        chrome_enabled = False

    result_code = _execute_step(
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


def flow(
    name: str = typer.Argument(help="Flow name from .lf/flows/"),
    path: list[str] = typer.Option(
        None, "-p", "-P", "--path", help="Additional files for all steps"
    ),
    worktree: str = typer.Option(
        None, "-w", "-W", "--worktree", help="Create worktree and run flow there"
    ),
    pr: bool = typer.Option(None, "--pr", help="Open PR when done"),
    web: bool = typer.Option(
        False, "--web", help="Copy to clipboard and open web client (claude.ai, chatgpt.com, etc.)"
    ),
    model: ModelType = typer.Option(
        None, "-m", "-M", "--model", help="Model to use (backend or backend:variant)"
    ),
):
    """Run a named flow."""
    repo_root = find_worktree_root()

    # Worktree creation still requires git
    if not repo_root and worktree:
        typer.echo("Error: --worktree requires a git repository", err=True)
        raise typer.Exit(1)

    # Use cwd as fallback for non-git usage
    if not repo_root:
        repo_root = Path.cwd()

    config = load_config(repo_root)

    flow_def = load_flow(name, repo_root)

    if not flow_def:
        typer.echo(f"Error: Flow '{name}' not found in .lf/flows/", err=True)
        raise typer.Exit(1)

    agent_model = model or (config.agent_model if config else "claude:opus")
    backend, model_variant = parse_model(agent_model)

    try:
        runner = get_runner(backend)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not web and not runner.is_available():
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
    if path:
        all_context.extend(path)

    exclude = list(config.exclude) if config and config.exclude else None

    if web:
        # Show tokens for first step in flow
        first_step = flow_def.steps[0].step if flow_def.steps else None

        if not first_step:
            typer.echo("Error: Flow has no steps", err=True)
            raise typer.Exit(1)

        # Determine diff_mode from config
        flow_diff_mode = DiffMode.FILES
        if config and not config.diff_files:
            flow_diff_mode = DiffMode.NONE
        elif config and config.diff:
            flow_diff_mode = DiffMode.DIFF

        components = gather_prompt_components(
            repo_root,
            first_step,
            context_config=ContextConfig(
                diff_mode=flow_diff_mode,
                files=FilesetConfig(
                    paths=list(all_context) if all_context else [],
                    exclude=list(exclude) if exclude else [],
                ),
                lfdocs=config.include_loopflow_doc if config else True,
            ),
            config=config,
        )
        components = trim_components_if_needed(components)
        prompt = format_prompt(components)
        copy_to_clipboard(prompt)
        tree = analyze_components(components)
        typer.echo(f"Flow '{name}' first step: {first_step}\n")
        typer.echo(tree.format())
        warn_if_context_too_large(tree)
        typer.echo("\nCopied to clipboard.")
        _open_web_client(backend)
        raise typer.Exit(0)

    push_enabled = config.push if config else False
    pr_enabled = pr if pr is not None else (config.pr if config else False)
    skip_permissions = config.yolo if config else False
    chrome_enabled = config.chrome if config else False

    exit_code = run_flow_def(
        flow_def,
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

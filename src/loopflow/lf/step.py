"""Step execution commands."""

import subprocess
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
from loopflow.lf.directions import parse_direction_arg, resolve_directions
from loopflow.lf.execution import ExecutionParams, execute_step
from loopflow.lf.flow import run_flow
from loopflow.lf.flows import load_flow
from loopflow.lf.frontmatter import StepConfig, resolve_step_config
from loopflow.lf.launcher import get_runner
from loopflow.lf.output import (
    copy_to_clipboard,
    trim_components_if_needed,
    warn_if_context_too_large,
)
from loopflow.lf.tokens import analyze_components
from loopflow.lf.worktrees import WorktreeError, create

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


def _run_step(
    step_name: str,
    repo_root: Path,
    components: PromptComponents,
    is_interactive: bool,
    backend: str,
    model_variant: str | None,
    skip_permissions: bool,
    chrome: bool = False,
) -> int:
    """Execute a single step."""
    return execute_step(
        ExecutionParams(
            step_name=step_name,
            repo_root=repo_root,
            components=components,
            backend=backend,
            model_variant=model_variant,
            skip_permissions=skip_permissions,
            chrome=chrome,
            is_interactive=is_interactive,
            use_execvp=is_interactive,  # Single-step interactive replaces process
            autocommit=True,
        )
    )


def _launch_interactive_default(
    repo_root: Path,
    config,
    context: list[str] | None = None,
    model: str | None = None,
    direction: str | None = None,
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
    direction_names = parse_direction_arg(direction) or (config.direction if config else None)
    resolved_direction = resolve_directions(repo_root, direction_names) if direction_names else None

    # Resolve flags
    include_clipboard = clipboard if clipboard is not None else (config.paste if config else False)
    include_docs = docs if docs is not None else (config.lfdocs if config else True)

    components = gather_prompt_components(
        repo_root,
        step=None,
        inline=None,
        run_mode="interactive",
        direction=resolved_direction,
        context_config=ContextConfig.for_interactive(
            paths=list(context) if context else [],
            exclude=list(config.exclude) if config and config.exclude else [],
            lfdocs=config.include_loopflow_doc if config else True,
            clipboard=include_clipboard,
        ),
        config=config,
    )

    # Apply docs flag
    if not include_docs:
        components.docs = []

    components = trim_components_if_needed(components)

    result_code = _run_step(
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
    area: list[str] = typer.Option(None, "--area", help="Area scope (paths to include in context)"),
    wave: Optional[str] = typer.Option(
        None, "--wave", help="Wave name for roadmap scoping (e.g., 'rust', 'enterprise')"
    ),
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
        None, "--lfdocs/--no-lfdocs", help="Include reports/, roadmap/, scratch/, and .md files"
    ),
    diff_mode: Optional[str] = typer.Option(
        None, "--diff-mode", help="How to include branch changes: files, diff, or none"
    ),
    model: ModelType = typer.Option(
        None, "-m", "-M", "--model", help="Model to use (backend or backend:variant)"
    ),
    direction: Optional[list[str]] = typer.Option(
        None, "-d", "-D", "--direction", help="Direction to apply (repeatable, or comma-separated)"
    ),
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
            context=list(area) if area else None,
            model=model,
            direction=direction,
            clipboard=clipboard,
            docs=docs,
        )

    # Gather step file to get frontmatter config
    step_file = gather_step(repo_root, step, config)
    frontmatter = step_file.config if step_file else StepConfig()

    # Parse direction arg
    cli_directions = parse_direction_arg(direction)

    # Resolve config: CLI > frontmatter > global > defaults
    resolved = resolve_step_config(
        step_name=step,
        global_config=config,
        frontmatter=frontmatter,
        cli_interactive=True if interactive else None,
        cli_auto=True if auto else None,
        cli_model=model,
        cli_context=list(area) if area else None,
        cli_direction=cli_directions or None,
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

    # Resolve direction to objects
    resolved_direction = (
        resolve_directions(repo_root, resolved.direction) if resolved.direction else None
    )

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
    components = gather_prompt_components(
        repo_root,
        step,
        step_args=args,
        run_mode="interactive" if is_interactive else "auto",
        direction=resolved_direction,
        context_config=ContextConfig(
            diff_mode=resolved_diff_mode,
            files=FilesetConfig(
                paths=list(resolved.context) if resolved.context else [],
                exclude=list(exclude_patterns) if exclude_patterns else [],
            ),
            area=resolved.area,
            wave=wave,
            lfdocs=config.include_loopflow_doc if config else True,
            clipboard=include_clipboard,
            budget_area=config.budgets.area if config else 50000,
            budget_docs=config.budgets.docs if config else 30000,
            budget_diff=config.budgets.diff if config else 20000,
        ),
        config=config,
    )

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

    result_code = _run_step(
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
    area: list[str] = typer.Option(None, "--area", help="Area scope (paths to include in context)"),
    wave: Optional[str] = typer.Option(
        None, "--wave", help="Wave name for roadmap scoping (e.g., 'rust', 'enterprise')"
    ),
    web: bool = typer.Option(
        False, "--web", help="Copy to clipboard and open web client (claude.ai, chatgpt.com, etc.)"
    ),
    clipboard: Optional[bool] = typer.Option(
        None, "-c", "-C", "--clipboard/--no-clipboard", help="Include clipboard content in prompt"
    ),
    docs: Optional[bool] = typer.Option(
        None, "--lfdocs/--no-lfdocs", help="Include reports/, roadmap/, scratch/, and .md files"
    ),
    diff_mode: Optional[str] = typer.Option(
        None, "--diff-mode", help="How to include branch changes: files, diff, or none"
    ),
    model: ModelType = typer.Option(
        None, "-m", "-M", "--model", help="Model to use (backend or backend:variant)"
    ),
    direction: Optional[list[str]] = typer.Option(
        None, "-d", "-D", "--direction", help="Direction to apply (repeatable, or comma-separated)"
    ),
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

    # Parse direction arg
    cli_directions = parse_direction_arg(direction)

    # Resolve config for inline prompts (no frontmatter)
    resolved = resolve_step_config(
        step_name="inline",
        global_config=config,
        frontmatter=StepConfig(),
        cli_interactive=True if interactive else None,
        cli_auto=True if auto else None,
        cli_model=model,
        cli_context=list(area) if area else None,
        cli_direction=cli_directions or None,
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

    # Resolve direction to objects
    resolved_direction = (
        resolve_directions(repo_root, resolved.direction) if resolved.direction else None
    )

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

    components = gather_prompt_components(
        repo_root,
        step=None,
        inline=prompt,
        run_mode="interactive" if is_interactive else "auto",
        direction=resolved_direction,
        context_config=ContextConfig(
            diff_mode=resolved_diff_mode,
            files=FilesetConfig(
                paths=list(resolved.context) if resolved.context else [],
                exclude=list(exclude_patterns) if exclude_patterns else [],
            ),
            area=resolved.area,
            wave=wave,
            lfdocs=config.include_loopflow_doc if config else True,
            clipboard=include_clipboard,
            budget_area=config.budgets.area if config else 50000,
            budget_docs=config.budgets.docs if config else 30000,
            budget_diff=config.budgets.diff if config else 20000,
        ),
        config=config,
    )

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

    result_code = _run_step(
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
    area: list[str] = typer.Option(None, "--area", help="Area scope (paths to include in context)"),
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

    loaded_flow = load_flow(name, repo_root)

    if not loaded_flow:
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
    if area:
        all_context.extend(area)

    exclude = list(config.exclude) if config and config.exclude else None

    if web:
        # Show tokens for first step in flow
        first_step = loaded_flow.steps[0].step if loaded_flow.steps else None

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

    exit_code = run_flow(
        loaded_flow,
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

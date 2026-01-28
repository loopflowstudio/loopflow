"""Copy context to clipboard."""

from pathlib import Path
from typing import Optional

import typer

from loopflow.lf.config import load_config
from loopflow.lf.context import (
    ContextConfig,
    DiffMode,
    FilesetConfig,
    find_worktree_root,
    format_prompt,
    gather_prompt_components,
)
from loopflow.lf.output import copy_to_clipboard, warn_if_context_too_large
from loopflow.lf.tokens import analyze_components


def register_commands(app: typer.Typer) -> None:
    """Register cp command on the app."""

    @app.command()
    def cp(
        paths: list[str] = typer.Argument(
            None, help="Files or directories to include (e.g., src tests)"
        ),
        exclude: list[str] = typer.Option(
            None, "-e", "-E", "--exclude", help="Patterns to exclude"
        ),
        clipboard: bool = typer.Option(
            False, "-c", "-C", "--clipboard", help="Include clipboard content"
        ),
        docs: Optional[bool] = typer.Option(
            None, "--lfdocs/--no-lfdocs", help="Include reports/, roadmap/, scratch/, and .md files"
        ),
        diff_mode: Optional[str] = typer.Option(
            None, "--diff-mode", help="How to include branch changes: files, diff, or none"
        ),
    ):
        """Copy file context to clipboard for use with web clients."""
        repo_root = find_worktree_root()
        if not repo_root:
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
            run_mode=None,
            context_config=ContextConfig(
                diff_mode=resolved_diff_mode,
                files=FilesetConfig(
                    paths=list(all_context) if all_context else [],
                    exclude=list(exclude_patterns) if exclude_patterns else [],
                ),
                lfdocs=config.include_loopflow_doc if config else True,
                clipboard=clipboard,
            ),
            config=config,
        )

        # Apply docs flag
        if not include_docs:
            components.docs = []

        prompt = format_prompt(components)
        copy_to_clipboard(prompt)

        tree = analyze_components(components)
        typer.echo(tree.format())
        warn_if_context_too_large(tree)
        typer.echo("\nCopied to clipboard.")

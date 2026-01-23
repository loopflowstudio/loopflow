"""Copy context to clipboard."""

from pathlib import Path
from typing import Optional

import typer

from loopflow.lf.config import load_config
from loopflow.lf.context import (
    ContextConfig,
    find_worktree_root,
    format_prompt,
    gather_prompt_components,
)
from loopflow.lf.output import (
    copy_to_clipboard,
    trim_components_if_needed,
    warn_if_context_too_large,
)
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
        paste: bool = typer.Option(False, "-v", "-V", "--paste", help="Include clipboard content"),
        docs: Optional[bool] = typer.Option(
            None, "--lfdocs/--no-lfdocs", help="Include .docs/, .design/, and root .md files"
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
        include_diff = diff if diff is not None else (config.diff if config else False)
        if diff_files is not None:
            include_diff_files = diff_files
        else:
            include_diff_files = config.diff_files if config else True
        include_summaries = (
            summaries if summaries is not None else bool(config and config.summaries)
        )

        components = gather_prompt_components(
            repo_root,
            step=None,
            run_mode=None,
            context_config=ContextConfig(
                pathset=list(all_context) if all_context else [],
                exclude=list(exclude_patterns) if exclude_patterns else [],
                lfdocs=config.include_loopflow_doc if config else True,
                diff=include_diff,
                diff_files=include_diff_files,
                summaries=include_summaries,
                clipboard=paste,
            ),
            config=config,
        )

        # Apply docs flag
        if not include_docs:
            components.docs = []

        components = trim_components_if_needed(components)

        prompt = format_prompt(components)
        copy_to_clipboard(prompt)

        tree = analyze_components(components)
        typer.echo(tree.format())
        warn_if_context_too_large(tree)
        typer.echo("\nCopied to clipboard.")

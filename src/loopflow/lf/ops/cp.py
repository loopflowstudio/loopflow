"""Copy context to clipboard."""

from pathlib import Path
from typing import Optional

import typer

from loopflow.lf.config import load_config
from loopflow.lf.context import (
    ContextConfig,
    DiffMode,
    FilesetConfig,
    PromptComponents,
    find_worktree_root,
    gather_prompt_components,
)
from loopflow.lf.output import copy_to_clipboard, warn_if_context_too_large
from loopflow.lf.tokens import analyze_components


def _format_files_raw(components: PromptComponents) -> str:
    """Format file content with <lf:file> tags but no instructional prompts."""
    all_files = list(components.docs) + list(components.diff_files)

    if not all_files and not components.diff and not components.clipboard:
        return ""

    parts = []
    for file_path, content in all_files:
        relative = file_path.relative_to(components.repo_root)
        parts.append(f'<lf:file path="{relative}">\n{content}\n</lf:file>')

    # Raw diff (if using diff mode instead of files mode)
    if components.diff:
        parts.append(f"<lf:diff>\n{components.diff}\n</lf:diff>")

    # Clipboard text
    if components.clipboard and components.clipboard.text:
        parts.append(f"<lf:clipboard>\n{components.clipboard.text}\n</lf:clipboard>")

    body = "\n\n".join(parts)
    return f"<lf:files>\n{body}\n</lf:files>"


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
                lfdocs=False,  # Never include loopflow docs for raw copy
                clipboard=clipboard,
            ),
            config=config,
        )

        # Apply docs flag
        if not include_docs:
            components.docs = []

        output = _format_files_raw(components)
        copy_to_clipboard(output)

        tree = analyze_components(components)
        typer.echo(tree.format())
        warn_if_context_too_large(tree)
        typer.echo("\nCopied to clipboard.")

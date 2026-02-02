"""Copy context to clipboard."""

from pathlib import Path
from typing import Optional

import typer

from loopflow.lf.config import extend_list, load_config, resolve_flag
from loopflow.lf.context import find_worktree_root, gather_diff_files
from loopflow.lf.design import gather_lfdocs
from loopflow.lf.files import format_files_raw, gather_files
from loopflow.lf.output import copy_to_clipboard, warn_if_context_too_large
from loopflow.lf.tokens import TokenTree, count_tokens
from loopflow.lf.wave import determine_wave


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
        lfdocs: Optional[bool] = typer.Option(
            None, "--lfdocs/--no-lfdocs", help="Include scratch/, root .md, and roadmap/<wave>/"
        ),
    ):
        """Copy file context to clipboard for use with web clients."""
        repo_root = find_worktree_root()
        if not repo_root:
            repo_root = Path.cwd()

        config = load_config(repo_root) if (repo_root / ".lf" / "config.yaml").exists() else None

        all_paths = extend_list(paths, config.context if config else None)
        if not all_paths:
            all_paths = gather_diff_files(repo_root)

        exclude_patterns = extend_list(exclude, config.exclude if config else None)
        include_lfdocs = resolve_flag(lfdocs, config.lfdocs if config else None, True)

        # Gather files from paths
        result = gather_files(all_paths, repo_root, exclude_patterns)
        files: list[tuple[Path, str]] = list(result.text_files)

        # Add lfdocs if enabled: scratch/, roadmap/<wave>/, root .md
        if include_lfdocs:
            wave_ctx = determine_wave(repo_root)
            wave_name = wave_ctx.name if wave_ctx else None
            seen = {p for p, _ in files}
            for path, content in gather_lfdocs(repo_root, wave=wave_name):
                if path not in seen:
                    seen.add(path)
                    files.append((path, content))

        # Note: clipboard flag defined for interface consistency with `lf step`
        # but not implemented - clipboard images require additional encoding work
        _ = clipboard  # unused, acknowledged

        output = format_files_raw(files, repo_root)

        copy_to_clipboard(output)

        # Build token tree for display
        tree = TokenTree()
        for file_path, content in files:
            tokens = count_tokens(content)
            rel = file_path.relative_to(repo_root)
            path_parts = list(rel.parts[:-1])
            tree.add("files", rel.name, tokens, path=path_parts)

        typer.echo(tree.format())
        warn_if_context_too_large(tree)
        typer.echo("\nCopied to clipboard.")

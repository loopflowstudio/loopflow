"""Copy context to clipboard."""

import subprocess
from pathlib import Path
from typing import Optional

import typer

from loopflow.lf.config import load_config
from loopflow.lf.context import find_worktree_root
from loopflow.lf.design import gather_lfdocs
from loopflow.lf.files import gather_files
from loopflow.lf.output import copy_to_clipboard, warn_if_context_too_large
from loopflow.lf.tokens import TokenTree, count_tokens
from loopflow.lf.wave import determine_wave


def _gather_diff_file_paths(repo_root: Path) -> list[str]:
    """Get paths of files changed on this branch vs main."""
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    branch = result.stdout.strip()
    if not branch or branch == "main":
        return []

    result = subprocess.run(
        ["git", "diff", "--name-only", "origin/main...HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return []

    return [
        line for line in result.stdout.strip().split("\n") if line and (repo_root / line).exists()
    ]


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

        # Merge positional paths and config context
        all_paths = list(paths or [])
        if config and config.context:
            all_paths.extend(config.context)

        # If no paths specified, use branch diff files
        if not all_paths:
            all_paths = _gather_diff_file_paths(repo_root)

        # Merge exclude patterns
        exclude_patterns = list(exclude or [])
        if config and config.exclude:
            exclude_patterns.extend(config.exclude)

        # Resolve lfdocs flag (CLI overrides config)
        include_lfdocs = lfdocs if lfdocs is not None else (config.lfdocs if config else True)

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

        # Format output
        parts = []
        for file_path, content in files:
            relative = file_path.relative_to(repo_root)
            parts.append(f'<lf:file path="{relative}">\n{content}\n</lf:file>')

        # Note: clipboard flag defined for interface consistency with `lf step`
        # but not implemented - clipboard images require additional encoding work
        _ = clipboard  # unused, acknowledged
        body = "\n\n".join(parts)
        output = f"<lf:files>\n{body}\n</lf:files>" if parts else ""

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

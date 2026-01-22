"""Output utilities for CLI commands."""

import subprocess

import typer

from loopflow.lf.context import (
    PromptComponents,
    format_drop_label,
    trim_prompt_components,
)
from loopflow.lf.tokens import MAX_SAFE_TOKENS


def copy_to_clipboard(text: str) -> None:
    """Copy text to clipboard using pbcopy."""
    subprocess.run(["pbcopy"], input=text.encode(), check=True)


def warn_if_context_too_large(tree) -> None:
    """Warn user if prompt exceeds safe token limit."""
    total_tokens = tree.total()
    if total_tokens > MAX_SAFE_TOKENS:
        typer.echo(
            f"\033[33m⚠ Prompt is {total_tokens:,} tokens (limit ~{MAX_SAFE_TOKENS:,})\033[0m",
            err=True,
        )
        files_node = tree.root.children.get("files")
        if files_node and files_node.total_tokens() > MAX_SAFE_TOKENS * 0.5:
            typer.echo(
                "\033[33m  Large branch - try: --no-diff-files or -x <specific files>\033[0m",
                err=True,
            )
        typer.echo(err=True)


def trim_components_if_needed(components: PromptComponents) -> PromptComponents:
    """Trim prompt components to fit within the safe token limit."""
    trimmed, dropped = trim_prompt_components(components, MAX_SAFE_TOKENS)
    if dropped:
        dropped_summary = ", ".join(format_drop_label(item) for item in dropped)
        typer.echo(
            f"\033[33m⚠ Context trimmed to fit {MAX_SAFE_TOKENS:,} tokens. "
            f"Dropped: {dropped_summary}\033[0m",
            err=True,
        )
    return trimmed

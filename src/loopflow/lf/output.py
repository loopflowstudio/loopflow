"""Output utilities for CLI commands."""

import subprocess

import typer

from loopflow.lf.context import (
    PromptComponents,
    format_drop_label,
    trim_prompt_components,
)
from loopflow.lf.tokens import MAX_SAFE_TOKENS


def print_step_header(
    step_name: str,
    backend: str,
    model_variant: str | None = None,
    direction: list[str] | None = None,
    context: list[str] | None = None,
    step_num: int | None = None,
    total_steps: int | None = None,
    token_summary: str | None = None,
) -> None:
    """Print step execution header with config summary and token profile.

    Used by both single-step execution (lf <step>) and flow execution (lf flow).
    """
    # Step header
    print(f"\n\033[1m{'─' * 60}\033[0m")
    if step_num is not None and total_steps is not None:
        print(f"\033[1m[{step_num}/{total_steps}] {step_name}\033[0m")
    else:
        print(f"\033[1m{step_name}\033[0m")
    print(f"\033[1m{'─' * 60}\033[0m")

    # Config summary
    model_str = f"{backend}"
    if model_variant:
        model_str += f":{model_variant}"
    config_parts = [f"model={model_str}"]
    if direction:
        config_parts.append(f"direction={','.join(direction)}")
    if context:
        config_parts.append(f"context={','.join(context)}")
    print(f"\033[90m{' | '.join(config_parts)}\033[0m\n")

    # Token profile
    if token_summary:
        for line in token_summary.split("\n"):
            print(f"\033[90m{line}\033[0m")
        print()


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

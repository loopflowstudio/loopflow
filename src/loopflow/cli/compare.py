"""Compare two worktree implementations."""

import subprocess
from pathlib import Path
from typing import Optional

import typer

from loopflow.config import load_config
from loopflow.context import find_worktree_root, gather_prompt_components, format_prompt
from loopflow.git import find_main_repo, get_current_branch
from loopflow.launcher import get_runner
from loopflow.worktrees import WorktreeError, get_path, list_all


def _get_diff_against_main(wt_path: Path) -> str:
    """Get diff of worktree against main branch."""
    result = subprocess.run(
        ["git", "diff", "main...HEAD"],
        cwd=wt_path,
        capture_output=True,
        text=True,
    )
    return result.stdout if result.returncode == 0 else ""


def _find_get_path(main_repo: Path, name: str) -> Path | None:
    """Find worktree path by name, checking both exact match and branch name."""
    # First try exact worktree path calculation
    wt_path = get_path(main_repo, name)
    if wt_path.exists():
        return wt_path

    # Try finding in worktree list
    try:
        worktrees = list_all(main_repo)
        for wt in worktrees:
            if wt.name == name or wt.branch == name:
                return wt.path
    except WorktreeError:
        pass

    return None


def compare(
    a: str = typer.Argument(help="First worktree name"),
    b: str = typer.Argument(help="Second worktree name"),
    print_mode: bool = typer.Option(
        False, "-p", "--print", help="Run non-interactively"
    ),
    model: Optional[str] = typer.Option(
        None, "-m", "--model", help="Model to use (claude, codex)"
    ),
    output: Optional[str] = typer.Option(
        None, "-o", "--output", help="Output file for analysis (default: <branch>.md)"
    ),
):
    """Compare two worktree implementations and analyze differences.

    Launches an LLM session to analyze the diffs from two worktrees.
    The analysis is written to a markdown file in the current directory.

    Examples:
        lf compare impl-claude impl-codex
        lf compare feature-a feature-b -p
        lf compare auth-v1 auth-v2 -o comparison.md
    """
    # Find main repo
    main_repo = find_main_repo()
    if not main_repo:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    # Resolve worktree paths
    wt_a = _find_get_path(main_repo, a)
    wt_b = _find_get_path(main_repo, b)

    if not wt_a:
        typer.echo(f"Error: Worktree '{a}' not found", err=True)
        raise typer.Exit(1)

    if not wt_b:
        typer.echo(f"Error: Worktree '{b}' not found", err=True)
        raise typer.Exit(1)

    # Get diffs against main for both worktrees
    diff_a = _get_diff_against_main(wt_a)
    diff_b = _get_diff_against_main(wt_b)

    if not diff_a and not diff_b:
        typer.echo("Error: Neither worktree has changes against main", err=True)
        raise typer.Exit(1)

    # Determine output file
    cwd = find_worktree_root() or Path.cwd()
    if output:
        output_file = output
    else:
        current_branch = get_current_branch(cwd)
        if current_branch and current_branch != "main":
            output_file = f"{current_branch}.md"
        else:
            output_file = "compare.md"

    # Load config
    config = load_config(main_repo)
    model_name = model or (config.model if config else "claude")
    skip_permissions = config.yolo if config else False

    # Get runner
    try:
        runner = get_runner(model_name)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not runner.is_available():
        typer.echo(f"Error: '{model_name}' CLI not found", err=True)
        raise typer.Exit(1)

    # Build prompt using template substitution
    exclude = list(config.exclude) if config and config.exclude else None
    task_args = [
        f"name_a={a}",
        f"name_b={b}",
        f"diff_a={diff_a}",
        f"diff_b={diff_b}",
        f"output_file={output_file}",
    ]

    components = gather_prompt_components(
        main_repo,
        task="compare",
        task_args=task_args,
        exclude=exclude,
    )
    prompt = format_prompt(components)

    # Launch runner
    typer.echo(f"Comparing {a} vs {b}...")
    if not print_mode:
        typer.echo(f"Analysis will be written to: {output_file}")

    result = runner.launch(
        prompt,
        print_mode=print_mode,
        stream=print_mode,
        skip_permissions=skip_permissions,
        cwd=cwd,
    )

    raise typer.Exit(result.exit_code)

"""Compare two worktree implementations."""

import subprocess
from pathlib import Path
from typing import Optional

import typer

from loopflow.config import load_config, parse_model
from loopflow.context import find_worktree_root, gather_prompt_components, format_prompt
from loopflow.git import find_main_repo, get_current_branch
from loopflow.launcher import get_runner
from loopflow.worktrees import WorktreeError, get_path, list_all


def _get_default_base_ref(repo_root: Path) -> str:
    """Resolve the default branch ref (origin/HEAD), with main fallback."""
    result = subprocess.run(
        ["git", "symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip()
    return "main"


def _get_diff_against_base(wt_path: Path, base_ref: str) -> str:
    """Get diff of a worktree against the base ref."""
    result = subprocess.run(
        ["git", "diff", f"{base_ref}...HEAD"],
        cwd=wt_path,
        capture_output=True,
        text=True,
    )
    return result.stdout if result.returncode == 0 else ""


def _find_worktree_path(main_repo: Path, name: str) -> Path | None:
    """Find worktree path by name."""
    try:
        worktrees = list_all(main_repo)
        for wt in worktrees:
            if wt.branch == name:
                return wt.path
    except WorktreeError:
        pass

    wt_path = get_path(main_repo, name)
    if wt_path.exists():
        return wt_path

    return None


def _find_branch_ref(main_repo: Path, name: str) -> str | None:
    """Find a branch ref for name (local preferred, then origin)."""
    result = subprocess.run(
        ["git", "show-ref", "--verify", "--quiet", f"refs/heads/{name}"],
        cwd=main_repo,
    )
    if result.returncode == 0:
        return name

    result = subprocess.run(
        ["git", "show-ref", "--verify", "--quiet", f"refs/remotes/origin/{name}"],
        cwd=main_repo,
    )
    if result.returncode == 0:
        return f"origin/{name}"

    return None


def _get_diff_for_target(main_repo: Path, name: str, base_ref: str) -> tuple[str, bool]:
    """Get diff for a worktree if present, otherwise a branch ref."""
    wt_path = _find_worktree_path(main_repo, name)
    if wt_path:
        return _get_diff_against_base(wt_path, base_ref), True

    branch_ref = _find_branch_ref(main_repo, name)
    if not branch_ref:
        return "", False

    result = subprocess.run(
        ["git", "diff", f"{base_ref}...{branch_ref}"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    return (result.stdout if result.returncode == 0 else ""), True


def compare(
    a: str = typer.Argument(help="First worktree name"),
    b: str = typer.Argument(help="Second worktree name"),
    print_mode: bool = typer.Option(
        False, "-p", "--print", help="Run non-interactively"
    ),
    model: Optional[str] = typer.Option(
        None, "-m", "--model", help="Model to use (backend or backend:variant)"
    ),
    output: Optional[str] = typer.Option(
        None, "-o", "--output", help="Output file for analysis (default: .design/)"
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

    base_ref = _get_default_base_ref(main_repo)
    diff_a, found_a = _get_diff_for_target(main_repo, a, base_ref)
    diff_b, found_b = _get_diff_for_target(main_repo, b, base_ref)

    if not found_a:
        typer.echo(f"Error: Worktree '{a}' not found", err=True)
        raise typer.Exit(1)

    if not found_b:
        typer.echo(f"Error: Worktree '{b}' not found", err=True)
        raise typer.Exit(1)

    if not diff_a and not diff_b:
        typer.echo("Error: No changes found for either worktree", err=True)
        raise typer.Exit(1)

    # Determine output destination
    cwd = find_worktree_root() or Path.cwd()
    if output:
        output_dir = output
    else:
        output_dir = ".design/"

    # Load config
    config = load_config(main_repo)
    agent_model = model or (config.agent_model if config else "claude:opus")
    backend, _model_variant = parse_model(agent_model)
    skip_permissions = config.yolo if config else False

    # Get runner
    try:
        runner = get_runner(backend)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not runner.is_available():
        typer.echo(f"Error: '{backend}' CLI not found", err=True)
        raise typer.Exit(1)

    # Build prompt using template substitution
    exclude = list(config.exclude) if config and config.exclude else None
    task_args = [
        f"name_a={a}",
        f"name_b={b}",
        f"diff_a={diff_a}",
        f"diff_b={diff_b}",
        f"output_dir={output_dir}",
    ]

    components = gather_prompt_components(
        main_repo,
        task="compare",
        task_args=task_args,
        exclude=exclude,
        include_tests_for=config.include_tests_for if config else None,
    )
    prompt = format_prompt(components)

    # Launch runner
    typer.echo(f"Comparing {a} vs {b}...")
    if not print_mode:
        if output:
            typer.echo(f"Analysis will be written to: {output_dir}")
        else:
            typer.echo("Analysis will be written under: .design/")

    result = runner.launch(
        prompt,
        auto=print_mode,
        stream=print_mode,
        skip_permissions=skip_permissions,
        cwd=cwd,
    )

    raise typer.Exit(result.exit_code)

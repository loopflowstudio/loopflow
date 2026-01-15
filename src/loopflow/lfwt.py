"""lfwt: Worktree operations CLI."""

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Optional

import typer

from loopflow.config import load_config, parse_model
from loopflow.context import find_worktree_root, gather_prompt_components, format_prompt
from loopflow.git import find_main_repo
from loopflow.launcher import get_runner
from loopflow.worktrees import (
    WorktreeError,
    diff_against,
    diff_between,
    get_github_compare_url,
    get_path,
    list_all,
)

app = typer.Typer(help="Worktree operations")


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


def _get_diff_for_target(main_repo: Path, name: str, base_ref: str) -> tuple[str, bool]:
    """Get diff for a worktree if present, otherwise a branch ref."""
    wt_path = _find_worktree_path(main_repo, name)
    if wt_path:
        result = subprocess.run(
            ["git", "diff", f"{base_ref}...HEAD"],
            cwd=wt_path,
            capture_output=True,
            text=True,
        )
        return result.stdout if result.returncode == 0 else "", True

    # Try as branch ref
    result = subprocess.run(
        ["git", "show-ref", "--verify", "--quiet", f"refs/heads/{name}"],
        cwd=main_repo,
    )
    if result.returncode == 0:
        branch_ref = name
    else:
        result = subprocess.run(
            ["git", "show-ref", "--verify", "--quiet", f"refs/remotes/origin/{name}"],
            cwd=main_repo,
        )
        if result.returncode == 0:
            branch_ref = f"origin/{name}"
        else:
            return "", False

    result = subprocess.run(
        ["git", "diff", f"{base_ref}...{branch_ref}"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    return (result.stdout if result.returncode == 0 else ""), True


@app.command(name="list")
def list_cmd() -> None:
    """Show all worktrees with status and PR info."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    main_repo = find_main_repo(repo_root) or repo_root

    try:
        worktrees = list_all(main_repo)
    except WorktreeError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not worktrees:
        typer.echo("No worktrees found")
        return

    # Print header
    typer.echo(f"{'BRANCH':<24} {'STATUS':<16} {'PR'}")
    typer.echo("-" * 60)

    for wt in worktrees:
        # Status column
        status_parts = []
        if wt.ahead_main > 0:
            status_parts.append(f"{wt.ahead_main}↑")
        else:
            status_parts.append("0↑")
        if wt.behind_main > 0:
            status_parts.append(f"{wt.behind_main}↓")
        else:
            status_parts.append("0↓")
        if wt.is_dirty:
            status_parts.append("dirty")
        status = " ".join(status_parts)

        # PR column
        if wt.pr_number:
            pr_info = f"#{wt.pr_number}"
            if wt.pr_state:
                pr_info += f" {wt.pr_state}"
        else:
            pr_info = "-"

        typer.echo(f"{wt.branch:<24} {status:<16} {pr_info}")


@app.command()
def diff(
    target: str = typer.Argument(..., help="Worktree or branch to diff"),
    other: Optional[str] = typer.Argument(None, help="Second worktree/branch (optional)"),
    base: str = typer.Option("main", "--base", "-b", help="Base branch for comparison"),
    web: bool = typer.Option(False, "-w", "--web", help="Open GitHub compare view"),
    terminal: bool = typer.Option(False, "-t", "--terminal", help="Print to terminal"),
) -> None:
    """Show diff for a worktree.

    By default opens in Cursor (or configured IDE).
    Use -w to open GitHub compare view.
    Use -t to print to terminal.
    """
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    main_repo = find_main_repo(repo_root) or repo_root
    config = load_config(main_repo)

    if other:
        # Diff between two worktrees/branches
        if web:
            typer.echo("Error: --web not supported for two-worktree diff", err=True)
            raise typer.Exit(1)

        diff_output = diff_between(main_repo, target, other)
        if not diff_output:
            typer.echo(f"No differences between {target} and {other}")
            return

        if terminal:
            typer.echo(diff_output)
        else:
            # Write to temp file and open in IDE
            fd, temp_path = tempfile.mkstemp(suffix=f"-{target}-vs-{other}.diff", prefix="lfwt-")
            diff_file = Path(temp_path)
            diff_file.write_text(diff_output)
            _open_in_ide(diff_file, config)
            typer.echo(f"Opened diff in IDE: {diff_file}")
    else:
        # Diff target against base
        if web:
            url = get_github_compare_url(main_repo, target, base)
            if url:
                subprocess.run(["open", url])
                typer.echo(f"Opened: {url}")
            else:
                typer.echo("Error: Could not determine GitHub URL", err=True)
                raise typer.Exit(1)
        elif terminal:
            diff_output = diff_against(main_repo, target, base)
            if diff_output:
                typer.echo(diff_output)
            else:
                typer.echo(f"No differences between {base} and {target}")
        else:
            # Open worktree in IDE (user sees Source Control diff)
            wt_path = _find_worktree_path(main_repo, target)
            if wt_path:
                _open_in_ide(wt_path, config)
                typer.echo(f"Opened {target} in IDE")
            else:
                # Fall back to generating diff file
                diff_output = diff_against(main_repo, target, base)
                if diff_output:
                    fd, temp_path = tempfile.mkstemp(suffix=f"-{target}.diff", prefix="lfwt-")
                    diff_file = Path(temp_path)
                    diff_file.write_text(diff_output)
                    _open_in_ide(diff_file, config)
                    typer.echo(f"Opened diff in IDE: {diff_file}")
                else:
                    typer.echo(f"No differences between {base} and {target}")


def _open_in_ide(path: Path, config) -> None:
    """Open path in configured IDE (default: Cursor)."""
    ide = "cursor"
    if config and config.ide and config.ide.cursor is False:
        ide = "code"  # Fall back to VS Code if Cursor disabled

    if shutil.which(ide):
        subprocess.run([ide, str(path)])
    elif shutil.which("code"):
        subprocess.run(["code", str(path)])
    else:
        typer.echo(f"Warning: {ide} not found, opening in Finder", err=True)
        subprocess.run(["open", str(path)])


@app.command()
def compare(
    a: str = typer.Argument(help="First worktree name"),
    b: str = typer.Argument(help="Second worktree name"),
    print_mode: bool = typer.Option(False, "-p", "--print", help="Run non-interactively"),
    model: Optional[str] = typer.Option(
        None, "-m", "--model", help="Model to use (backend or backend:variant)"
    ),
    output: Optional[str] = typer.Option(
        None, "-o", "--output", help="Output file for analysis (default: .design/)"
    ),
) -> None:
    """Compare two worktree implementations and analyze differences.

    Launches an LLM session to analyze the diffs from two worktrees.
    """
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

    cwd = find_worktree_root() or Path.cwd()
    output_dir = output or ".design/"

    config = load_config(main_repo)
    agent_model = model or (config.agent_model if config else "claude:opus")
    backend, _model_variant = parse_model(agent_model)
    skip_permissions = config.yolo if config else False

    try:
        runner = get_runner(backend)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not runner.is_available():
        typer.echo(f"Error: '{backend}' CLI not found", err=True)
        raise typer.Exit(1)

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

    typer.echo(f"Comparing {a} vs {b}...")
    if not print_mode:
        typer.echo(f"Analysis will be written to: {output_dir}")

    result = runner.launch(
        prompt,
        auto=print_mode,
        stream=print_mode,
        skip_permissions=skip_permissions,
        cwd=cwd,
    )

    raise typer.Exit(result.exit_code)


@app.command()
def cd(
    name: str = typer.Argument(help="Worktree name"),
) -> None:
    """Print path to worktree (for shell integration).

    Usage: cd $(lfwt cd feature-name)
    """
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    main_repo = find_main_repo(repo_root) or repo_root
    wt_path = _find_worktree_path(main_repo, name)

    if wt_path:
        typer.echo(str(wt_path))
    else:
        typer.echo(f"Error: Worktree '{name}' not found", err=True)
        raise typer.Exit(1)


def main() -> None:
    """Entry point for lfwt command."""
    if len(sys.argv) == 1:
        sys.argv.append("list")
    app()


if __name__ == "__main__":
    main()

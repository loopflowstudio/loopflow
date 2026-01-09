"""Worktree management commands."""

import subprocess

import typer

from pathlib import Path

from loopflow.config import Config, load_config
from loopflow.context import gather_prompt_components, format_prompt
from loopflow.git import GitError, WorktreeInfo, create_worktree, find_main_repo, list_worktrees, remove_worktree, worktree_path
from loopflow.launcher import get_runner

app = typer.Typer(help="Worktree management.")


def _find_workspace(wt_path: Path, config: Config | None) -> Path | None:
    """Find workspace file in the worktree."""
    if config and config.ide.workspace:
        workspace_path = wt_path / config.ide.workspace
        if workspace_path.exists():
            return workspace_path
        return None

    workspaces = list(wt_path.glob("*.code-workspace"))
    if len(workspaces) == 1:
        return workspaces[0]

    return None


def _open_ide(wt_path: Path, config: Config | None) -> None:
    """Open configured IDEs at worktree path."""
    ide = config.ide if config else None

    if not ide or ide.warp:
        subprocess.run(["open", f"warp://action/new_window?path={wt_path}"])

    if not ide or ide.cursor:
        workspace = _find_workspace(wt_path, config)
        if workspace:
            subprocess.run(["cursor", str(workspace)])
        else:
            subprocess.run(["cursor", str(wt_path)])


@app.command()
def create(
    name: str = typer.Argument(help="Branch/worktree name"),
):
    """Create a worktree and branch, open IDEs."""
    main_repo = find_main_repo()
    if not main_repo:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    try:
        wt_path = create_worktree(main_repo, name)
    except GitError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    config = load_config(main_repo)
    _open_ide(wt_path, config)
    typer.echo(f"cd {wt_path}")


@app.command(name="open")
def open_cmd(
    name: str = typer.Argument(help="Branch/worktree name"),
):
    """Open IDEs at an existing worktree."""
    main_repo = find_main_repo()
    if not main_repo:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    wt_path = worktree_path(main_repo, name)
    if not wt_path.exists():
        typer.echo(f"Error: Worktree '{name}' not found at {wt_path}", err=True)
        raise typer.Exit(1)

    config = load_config(main_repo)
    _open_ide(wt_path, config)
    typer.echo(f"cd {wt_path}")


def _link(url: str, text: str) -> str:
    """Format as clickable terminal hyperlink (OSC 8)."""
    return f"\033]8;;{url}\033\\{text}\033]8;;\033\\"


def _dim(text: str) -> str:
    """Apply dim styling to text."""
    return f"\033[2m{text}\033[0m"


def _format_status_symbols(wt: WorktreeInfo) -> str:
    """Format status symbols: +!?⤴⤵ for staged/modified/untracked/rebase/merge."""
    symbols = ""
    if wt.has_staged:
        symbols += "+"
    if wt.has_modified:
        symbols += "!"
    if wt.has_untracked:
        symbols += "?"
    if wt.is_rebasing:
        symbols += "⤴"
    if wt.is_merging:
        symbols += "⤵"
    return symbols


def _format_main_rel(wt: WorktreeInfo) -> str:
    """Format ahead/behind main like ↑3↓1."""
    parts = []
    if wt.ahead_main > 0:
        parts.append(f"↑{wt.ahead_main}")
    if wt.behind_main > 0:
        parts.append(f"↓{wt.behind_main}")
    return "".join(parts) if parts else "="


def _format_pr(wt: WorktreeInfo) -> str:
    """Format PR info with clickable hyperlink if available."""
    if wt.pr_number:
        text = f"#{wt.pr_number}"
        if wt.pr_url:
            return _link(wt.pr_url, text)
        return text
    elif wt.on_origin:
        return "pushed"
    return "local"


def _format_ci(wt: WorktreeInfo) -> str:
    """Format CI status with clickable hyperlink if available."""
    if not wt.ci_status:
        return "-"
    # Link to checks page if we have a PR URL
    if wt.pr_url and wt.ci_status != "-":
        checks_url = f"{wt.pr_url}/checks"
        return _link(checks_url, wt.ci_status)
    return wt.ci_status


def _format_remote_rel(wt: WorktreeInfo) -> str:
    """Format ahead/behind remote like ↑3↓1 or = if in sync."""
    if not wt.on_origin:
        return "-"
    parts = []
    if wt.ahead_remote > 0:
        parts.append(f"↑{wt.ahead_remote}")
    if wt.behind_remote > 0:
        parts.append(f"↓{wt.behind_remote}")
    return "".join(parts) if parts else "="


def _format_diff_stats(wt: WorktreeInfo) -> str:
    """Format line diff stats like +45 -12."""
    if wt.lines_added == 0 and wt.lines_removed == 0:
        return "-"
    parts = []
    if wt.lines_added > 0:
        parts.append(f"+{wt.lines_added}")
    if wt.lines_removed > 0:
        parts.append(f"-{wt.lines_removed}")
    return " ".join(parts)


@app.command(name="list")
def list_cmd(
    full: bool = typer.Option(False, "--full", "-f", help="Show full details including commit message"),
    json_output: bool = typer.Option(False, "--json", help="Output as JSON"),
):
    """List all worktrees with status and dependencies.

    Columns:
    - St: Status symbols (+!?⤴⤵) for staged/modified/untracked/rebase/merge
    - main: Ahead/behind main (↑N, ↓N, = for in sync)
    - remote: Ahead/behind remote tracking branch (↑N, ↓N, = for in sync, - if local)
    - CI: CI status (✓ passed, ✗ failed, ● running, - none)
    - PR: PR number (clickable link) or "pushed"/"local"
    - Diff: Line diff stats vs main (+N -M)

    Dimmed rows indicate worktrees safe to delete (merged or branch gone).
    """
    import json

    main_repo = find_main_repo()
    if not main_repo:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    worktrees = list_worktrees(main_repo)
    if not worktrees:
        if json_output:
            typer.echo("[]")
        else:
            typer.echo("No worktrees found")
        return

    # JSON output
    if json_output:
        data = []
        for wt in worktrees:
            data.append({
                "name": wt.name,
                "branch": wt.branch,
                "path": str(wt.path),
                "on_origin": wt.on_origin,
                "is_dirty": wt.is_dirty,
                "base_branch": wt.base_branch,
                "pr_url": wt.pr_url,
                "pr_number": wt.pr_number,
                "commit_sha": wt.commit_sha,
                "commit_age": wt.commit_age,
                "commit_message": wt.commit_message,
                "ahead_main": wt.ahead_main,
                "behind_main": wt.behind_main,
                "has_staged": wt.has_staged,
                "has_modified": wt.has_modified,
                "has_untracked": wt.has_untracked,
                "ci_status": wt.ci_status,
                "safe_to_delete": wt.safe_to_delete,
                "ahead_remote": wt.ahead_remote,
                "behind_remote": wt.behind_remote,
                "lines_added": wt.lines_added,
                "lines_removed": wt.lines_removed,
                "is_rebasing": wt.is_rebasing,
                "is_merging": wt.is_merging,
            })
        typer.echo(json.dumps(data, indent=2))
        return

    # Build dependency tree: group worktrees by their base branch
    by_base: dict[str, list] = {}
    for wt in worktrees:
        base = wt.base_branch or "main"
        by_base.setdefault(base, []).append(wt)

    # Calculate column widths dynamically
    max_name = max(len(wt.name) for wt in worktrees)
    max_tree_indent = 8  # space for tree chars like "└─ "
    max_name = max(max_name + max_tree_indent, 12)

    def format_row(wt: WorktreeInfo, tree_prefix: str = "") -> str:
        """Format a single worktree row."""
        status = _format_status_symbols(wt)
        main_rel = _format_main_rel(wt)
        remote_rel = _format_remote_rel(wt)
        ci = _format_ci(wt)
        pr = _format_pr(wt)
        diff = _format_diff_stats(wt)
        age = wt.commit_age or ""
        sha = wt.commit_sha or ""

        # Build the row
        name_col = f"{tree_prefix}{wt.name}"
        name_col = name_col.ljust(max_name)

        status_col = status.ljust(5)
        main_col = main_rel.ljust(6)
        remote_col = remote_rel.ljust(6)
        ci_col = ci.ljust(2)
        pr_col = pr.ljust(8)
        diff_col = diff.ljust(10)
        sha_col = sha.ljust(8)
        age_col = age.ljust(14)

        if full and wt.commit_message:
            row = f"{name_col}  {status_col}  {main_col}  {remote_col}  {ci_col}  {pr_col}  {diff_col}  {sha_col}  {age_col}  {wt.commit_message}"
        else:
            row = f"{name_col}  {status_col}  {main_col}  {remote_col}  {ci_col}  {pr_col}  {diff_col}  {sha_col}  {age_col}"

        # Dim rows for worktrees that are safe to delete
        if wt.safe_to_delete:
            return _dim(row)
        return row

    def print_header():
        name_col = "Branch".ljust(max_name)
        status_col = "St".ljust(5)
        main_col = "main".ljust(6)
        remote_col = "remote".ljust(6)
        ci_col = "CI".ljust(2)
        pr_col = "PR".ljust(8)
        diff_col = "Diff".ljust(10)
        sha_col = "Commit".ljust(8)
        age_col = "Age".ljust(14)

        if full:
            typer.echo(f"{name_col}  {status_col}  {main_col}  {remote_col}  {ci_col}  {pr_col}  {diff_col}  {sha_col}  {age_col}  Message")
        else:
            typer.echo(f"{name_col}  {status_col}  {main_col}  {remote_col}  {ci_col}  {pr_col}  {diff_col}  {sha_col}  {age_col}")

    def print_tree(parent: str, indent: int, printed: set, connector: str = ""):
        children = by_base.get(parent, [])
        for i, wt in enumerate(children):
            if wt.name in printed:
                continue
            printed.add(wt.name)

            # Build tree prefix
            is_last = (i == len(children) - 1)
            if indent == 0:
                tree_prefix = ""
                child_connector = ""
            else:
                tree_prefix = connector + ("└─ " if is_last else "├─ ")
                child_connector = connector + ("   " if is_last else "│  ")

            typer.echo(format_row(wt, tree_prefix))
            print_tree(wt.name, indent + 1, printed, child_connector)

    print_header()
    printed: set[str] = set()

    # Print branches rooted at main
    if "main" in by_base:
        print_tree("main", 0, printed)

    # Print branches with other bases (dependent on non-main branches that may not be worktrees)
    for base, children in by_base.items():
        if base == "main":
            continue
        # Check if base is itself a worktree we've printed
        if base not in printed:
            # It's a base branch that isn't a worktree
            typer.echo(f"\n{base} (external)")
        for wt in children:
            if wt.name not in printed:
                printed.add(wt.name)
                typer.echo(format_row(wt, "└─ "))
                print_tree(wt.name, 2, printed, "   ")

    # Summary
    dirty_count = sum(1 for wt in worktrees if wt.is_dirty)
    pr_count = sum(1 for wt in worktrees if wt.pr_number)
    typer.echo(f"\n{len(worktrees)} worktrees, {pr_count} with PRs, {dirty_count} dirty")


@app.command()
def clean(
    force: bool = typer.Option(False, "-f", "--force", help="Skip confirmation"),
):
    """Remove worktrees for branches no longer on origin."""
    main_repo = find_main_repo()
    if not main_repo:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    # Prune stale remote-tracking branches
    subprocess.run(["git", "fetch", "--prune"], cwd=main_repo, capture_output=True)

    worktrees = list_worktrees(main_repo)
    to_remove = [wt for wt in worktrees if not wt.on_origin and not wt.is_dirty]

    if not to_remove:
        typer.echo("No worktrees to clean")
        return

    typer.echo("Worktrees to remove:")
    for wt in to_remove:
        typer.echo(f"  {wt.name}")

    if not force:
        confirm = typer.confirm("Remove these worktrees?")
        if not confirm:
            raise typer.Exit(0)

    for wt in to_remove:
        if remove_worktree(main_repo, wt.name):
            typer.echo(f"Removed {wt.name}")
        else:
            typer.echo(f"Failed to remove {wt.name}", err=True)


@app.command()
def compare(
    a: str = typer.Argument(help="First worktree name"),
    b: str = typer.Argument(help="Second worktree name"),
    model: str = typer.Option("claude", "-m", "--model", help="Model to use for analysis"),
):
    """Compare two worktree implementations and analyze differences."""
    main_repo = find_main_repo()
    if not main_repo:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    wt_a = worktree_path(main_repo, a)
    wt_b = worktree_path(main_repo, b)

    if not wt_a.exists():
        typer.echo(f"Error: Worktree '{a}' not found", err=True)
        raise typer.Exit(1)

    if not wt_b.exists():
        typer.echo(f"Error: Worktree '{b}' not found", err=True)
        raise typer.Exit(1)

    # Get diffs against main for both worktrees
    diff_a = subprocess.run(
        ["git", "diff", "main...HEAD"],
        cwd=wt_a,
        capture_output=True,
        text=True,
    ).stdout

    diff_b = subprocess.run(
        ["git", "diff", "main...HEAD"],
        cwd=wt_b,
        capture_output=True,
        text=True,
    ).stdout

    # Gather compare task components
    config = load_config(main_repo)
    exclude = list(config.exclude) if config and config.exclude else None

    # Prepare task args with the diffs
    task_args = [
        f"name_a={a}",
        f"name_b={b}",
        f"diff_a={diff_a}",
        f"diff_b={diff_b}",
    ]

    components = gather_prompt_components(
        main_repo,
        task="compare",
        task_args=task_args,
        exclude=exclude,
    )

    prompt = format_prompt(components)

    # Launch runner
    try:
        runner = get_runner(model)
    except ValueError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    if not runner.is_available():
        typer.echo(f"Error: '{model}' CLI not found", err=True)
        raise typer.Exit(1)

    skip_permissions = config.yolo if config else False
    result = runner.launch(
        prompt,
        print_mode=False,
        skip_permissions=skip_permissions,
        cwd=main_repo,
    )

    raise typer.Exit(result.exit_code)

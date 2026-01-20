"""Worktree proxy commands for lfops."""

import json
import subprocess
from pathlib import Path

import typer

from loopflow.lf.context import find_worktree_root
from loopflow.lf.worktrees import find_merged
from loopflow.lfops._helpers import get_default_branch, sync_main_repo


def register_commands(app: typer.Typer) -> None:
    wt_app = typer.Typer(help="Worktree helper commands")

    @wt_app.command("list")
    def list_worktrees(
        format: str = typer.Option("json", "--format", help="Output format"),
        full: bool = typer.Option(False, "--full", help="Include full details"),
        sync: bool = typer.Option(True, "--sync/--no-sync", help="Sync base branch first"),
    ) -> None:
        """List worktrees with prunable metadata."""
        if format != "json":
            typer.echo("Error: only --format json is supported", err=True)
            raise typer.Exit(1)

        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        base_branch = get_default_branch(repo_root)
        if sync:
            sync_main_repo(repo_root, base_branch)

        merged = {wt.branch for wt in find_merged(repo_root, base_branch)}
        items = _load_wt_list(repo_root, full)

        for item in items:
            branch = item.get("branch", "")
            item["prunable"] = branch in merged

        typer.echo(json.dumps(items))

    app.add_typer(wt_app, name="wt")


def _load_wt_list(repo_root: Path, full: bool) -> list[dict]:
    args = ["wt", "-C", str(repo_root), "list", "--format", "json"]
    if full:
        args.append("--full")
    result = subprocess.run(args, cwd=repo_root, capture_output=True, text=True)
    if result.returncode != 0:
        error = result.stderr.strip() or result.stdout.strip() or "Worktree operation failed"
        typer.echo(error, err=True)
        raise typer.Exit(1)

    if not result.stdout.strip():
        return []

    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError:
        typer.echo("Error: Could not parse worktree list JSON", err=True)
        raise typer.Exit(1)

"""Prune command for removing merged worktrees."""

import subprocess

import typer

from loopflow.lf.context import find_worktree_root
from loopflow.lf.worktrees import find_merged, list_all, merge_diagnostics, remove
from loopflow.lfops._helpers import get_default_branch, sync_main_repo


def _get_current_worktree_branch() -> str | None:
    """Get branch name of current worktree, if in one."""
    result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "HEAD"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    return result.stdout.strip()


def register_commands(app: typer.Typer) -> None:
    @app.command()
    def prune(
        dry_run: bool = typer.Option(False, "--dry-run", "-n", help="Show what would be pruned"),
        force: bool = typer.Option(False, "--force", "-f", help="Skip confirmation"),
        debug: bool = typer.Option(False, "--debug", help="Show merge detection details"),
    ) -> None:
        """Remove worktrees whose changes have been merged into main."""
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        # Sync main first so merge detection is accurate
        base_branch = get_default_branch(repo_root)
        typer.echo(f"Syncing {base_branch}...")
        sync_main_repo(repo_root, base_branch)

        # Find merged worktrees
        merged = find_merged(repo_root, base_branch)
        if debug:
            typer.echo("Merge diagnostics:")
            for wt in list_all(repo_root):
                info = merge_diagnostics(repo_root, wt, base_branch)
                typer.echo(
                    f"  {info['branch']}: "
                    f"dirty={info['is_dirty']} "
                    f"pr_state={info['pr_state'] or 'none'} "
                    f"cherry_empty={info['cherry_empty']} "
                    f"trees_match={info['trees_match']} "
                    f"is_ancestor={info['is_ancestor']}"
                )

        # Never prune the current worktree - user might be standing in it
        current_branch = _get_current_worktree_branch()
        if current_branch:
            merged = [wt for wt in merged if wt.branch != current_branch]

        if not merged:
            typer.echo("No merged worktrees found")
            return

        # Show what would be pruned
        if dry_run:
            typer.echo("Would remove:")
            for wt in merged:
                typer.echo(f"  {wt.branch}")
            return

        # Confirm unless forced
        if not force:
            typer.echo("The following worktrees will be removed:")
            for wt in merged:
                typer.echo(f"  {wt.branch}")
            confirm = typer.confirm("Proceed?")
            if not confirm:
                typer.echo("Aborted")
                raise typer.Exit(0)

        # Remove merged worktrees
        removed = []
        failed = []
        for wt in merged:
            if remove(repo_root, wt.branch):
                removed.append(wt.branch)
            else:
                failed.append(wt.branch)

        if removed:
            typer.echo("Removed:")
            for branch in removed:
                typer.echo(f"  {branch}")

        if failed:
            typer.echo("Failed to remove:", err=True)
            for branch in failed:
                typer.echo(f"  {branch}", err=True)
            raise typer.Exit(1)

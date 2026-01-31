"""Worktree proxy commands for lf ops."""

import json
import subprocess
from pathlib import Path
from typing import Annotated

import typer

from loopflow.lf.config import load_config
from loopflow.lf.context import find_worktree_root
from loopflow.lf.deps import require_wt
from loopflow.lf.git import find_main_repo
from loopflow.lf.worktrees import (
    create_with_schema,
    find_merged,
    get_path,
    list_all,
    merge_diagnostics,
    remove,
)
from loopflow.lf.ops._helpers import get_default_branch, sync_main_repo
from loopflow.lf.ops.shell import write_directive


def register_commands(app: typer.Typer) -> None:
    wt_app = typer.Typer(help="Worktree helper commands")

    @wt_app.command("create")
    def create_worktree(
        name: Annotated[str, typer.Argument(help="Short name for worktree")],
        base: Annotated[str | None, typer.Option("--base", "-b", help="Base branch")] = None,
        stack: Annotated[
            bool, typer.Option("--stack", "-s", help="Stack on current branch")
        ] = False,
    ) -> None:
        """Create worktree with schema-based branch name.

        The worktree directory uses the short NAME you provide.
        The git branch uses your configured schema (if any).

        Use --stack to branch from the current branch (stacking):

            lf ops wt create feature-B --stack
            # Creates worktree branched from current branch
            # PR will target current branch until it merges

        Example:
            lf ops wt create my-feature
            # Worktree: ../repo.my-feature
            # Branch: jack.my-feature.20260120_1234 (with schema)
        """
        require_wt()

        repo_root = find_main_repo()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        # Handle --stack: use current branch as base
        if stack:
            result = subprocess.run(
                ["git", "branch", "--show-current"],
                capture_output=True,
                text=True,
            )
            if result.returncode != 0 or not result.stdout.strip():
                typer.echo("Error: not on a branch (required for --stack)", err=True)
                raise typer.Exit(1)
            current_branch = result.stdout.strip()
            if current_branch in ("main", "master"):
                typer.echo("Error: cannot stack on main/master", err=True)
                raise typer.Exit(1)
            base = current_branch

        config = load_config(repo_root)
        branch_config = config.branch_names if config else None

        try:
            wt_result = create_with_schema(repo_root, name, base, branch_config)
        except Exception as e:
            typer.echo(f"Error: {e}", err=True)
            raise typer.Exit(1)

        typer.echo(f"Created worktree: {wt_result.path.name}")
        if wt_result.branch != name:
            typer.echo(f"Branch: {wt_result.branch}")
        if wt_result.base_branch:
            typer.echo(f"Base: {wt_result.base_branch}")

        # Write cd directive for shell integration
        if not write_directive(f"cd {wt_result.path}"):
            # Shell integration not active - print manual cd command
            typer.echo(f"\ncd {wt_result.path}")
            typer.echo("\nTip: Run 'lf ops shell install' for auto-cd after worktree creation")

    @wt_app.command("switch")
    def switch_worktree(
        name: Annotated[str, typer.Argument(help="Worktree name (directory suffix)")],
    ) -> None:
        """Switch to a worktree by its short directory name.

        Finds worktree by matching the directory suffix. For example,
        'concerto' matches '../loopflow.concerto'.

        Example:
            lf ops wt switch concerto
            # Switches to ../loopflow.concerto
        """
        repo_root = find_main_repo()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        # Find worktree by directory name
        target_path = get_path(repo_root, name)
        if not target_path.exists():
            # Try matching against existing worktrees
            worktrees = list_all(repo_root)
            matches = [wt for wt in worktrees if wt.path.name.endswith(f".{name}")]
            if len(matches) == 1:
                target_path = matches[0].path
            elif len(matches) > 1:
                typer.echo(f"Error: Multiple worktrees match '{name}':", err=True)
                for wt in matches:
                    typer.echo(f"  {wt.path.name}", err=True)
                raise typer.Exit(1)
            else:
                typer.echo(f"Error: No worktree found for '{name}'", err=True)
                typer.echo(f"Expected: {target_path}")
                raise typer.Exit(1)

        # Write cd directive for shell integration
        if not write_directive(f"cd {target_path}"):
            typer.echo(f"cd {target_path}")

    @wt_app.command("ci")
    def ci_status(
        watch: bool = typer.Option(False, "--watch", "-w", help="Watch until complete"),
        logs: bool = typer.Option(False, "--logs", "-l", help="Show logs for failed checks"),
    ) -> None:
        """Show CI status for the current branch."""
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        # Get current branch
        result = subprocess.run(
            ["git", "branch", "--show-current"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        branch = result.stdout.strip()
        if not branch:
            typer.echo("Error: Not on a branch", err=True)
            raise typer.Exit(1)

        # Check for PR and its checks
        args = ["gh", "pr", "checks", branch]
        if watch:
            args.append("--watch")
        result = subprocess.run(args, cwd=repo_root)

        # If there were failures and logs requested, fetch the logs
        if result.returncode != 0 and logs:
            typer.echo("\n--- Failed check logs ---\n")
            _show_failed_logs(repo_root, branch)

        raise typer.Exit(result.returncode)

    def _show_failed_logs(repo_root: Path, branch: str) -> None:
        """Fetch and display logs for failed CI checks."""
        # Get the run ID for the PR
        result = subprocess.run(
            [
                "gh",
                "pr",
                "view",
                branch,
                "--json",
                "statusCheckRollup",
                "-q",
                '.statusCheckRollup[] | select(.conclusion == "FAILURE" '
                'or .conclusion == "failure") | .detailsUrl',
            ],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0 or not result.stdout.strip():
            typer.echo("Could not find failed check details", err=True)
            return

        # Extract run IDs from URLs like https://github.com/.../actions/runs/123/job/456
        for url in result.stdout.strip().split("\n"):
            if not url:
                continue
            # Extract run ID from URL
            if "/actions/runs/" in url:
                parts = url.split("/actions/runs/")
                if len(parts) > 1:
                    run_part = parts[1].split("/")[0]
                    typer.echo(f"Run: {run_part}")
                    # Get failed logs
                    log_result = subprocess.run(
                        ["gh", "run", "view", run_part, "--log-failed"],
                        cwd=repo_root,
                        capture_output=True,
                        text=True,
                    )
                    if log_result.returncode == 0:
                        # Only show last 50 lines of each failed log
                        lines = log_result.stdout.strip().split("\n")
                        if len(lines) > 50:
                            typer.echo(f"... ({len(lines) - 50} lines truncated)")
                            lines = lines[-50:]
                        typer.echo("\n".join(lines))
                    else:
                        typer.echo(f"Failed to fetch logs: {log_result.stderr}", err=True)

    @wt_app.command("list")
    def list_worktrees(
        format: str = typer.Option("json", "--format", help="Output format"),
        full: bool = typer.Option(False, "--full", help="Include full details"),
        sync: bool = typer.Option(True, "--sync/--no-sync", help="Sync base branch first"),
    ) -> None:
        """List worktrees with prunable metadata."""
        require_wt()

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

    @wt_app.command("prune")
    def prune(
        dry_run: bool = typer.Option(False, "--dry-run", "-n", help="Show what would be pruned"),
        force: bool = typer.Option(False, "--force", "-f", help="Skip confirmation"),
        debug: bool = typer.Option(False, "--debug", help="Show merge detection details"),
    ) -> None:
        """Remove worktrees whose changes have been merged into main."""
        require_wt()
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

    app.add_typer(wt_app, name="wt")


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

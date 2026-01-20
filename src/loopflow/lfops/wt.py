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
            ["gh", "pr", "view", branch, "--json", "statusCheckRollup", "-q",
             '.statusCheckRollup[] | select(.conclusion == "FAILURE" or .conclusion == "failure") | .detailsUrl'],
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

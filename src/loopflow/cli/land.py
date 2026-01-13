"""Local landing workflow using worktrunk."""

import shutil
import subprocess
from pathlib import Path

import typer

from loopflow.config import load_config
from loopflow.context import find_worktree_root
from loopflow.design import clear_design_artifacts, has_design_artifacts
from loopflow.git import find_main_repo, get_current_branch
from loopflow.llm_http import generate_commit_message_from_diff

app = typer.Typer(help="Local landing workflow.")


def _get_default_branch(repo_root: Path) -> str:
    result = subprocess.run(
        ["git", "symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip().split("/", 1)[-1]
    return "main"


def _resolve_base_ref(repo_root: Path, base_branch: str) -> str:
    origin_ref = f"origin/{base_branch}"
    result = subprocess.run(
        ["git", "rev-parse", "--verify", origin_ref],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return origin_ref
    return base_branch


def _get_diff(repo_root: Path, base_ref: str) -> str:
    result = subprocess.run(
        ["git", "diff", f"{base_ref}...HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    return result.stdout if result.returncode == 0 else ""


def _get_pr_status(repo_root: Path) -> bool | None:
    if not shutil.which("gh"):
        return None

    result = subprocess.run(
        ["gh", "pr", "view", "--json", "url", "-q", ".url"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return True
    if "no pull requests" in (result.stderr or "").lower():
        return False
    return None


def _ensure_clean(repo_root: Path) -> None:
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        typer.echo("Error: Working tree has uncommitted changes", err=True)
        raise typer.Exit(1)


def _clear_design_artifacts(repo_root: Path) -> bool:
    return clear_design_artifacts(repo_root)


def _squash_commits(repo_root: Path, base_ref: str, commit_msg: str) -> None:
    original_head = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()

    subprocess.run(["git", "reset", "--soft", base_ref], cwd=repo_root, check=True)
    design_dir = repo_root / ".design"
    if design_dir.exists():
        subprocess.run(["git", "add", "-A", str(design_dir)], cwd=repo_root, check=False)

    staged = subprocess.run(
        ["git", "diff", "--cached", "--quiet"],
        cwd=repo_root,
    )
    if staged.returncode == 0:
        subprocess.run(["git", "reset", "--hard", original_head], cwd=repo_root, check=True)
        typer.echo("Error: Nothing to land after squash", err=True)
        raise typer.Exit(1)

    subprocess.run(["git", "commit", "-m", commit_msg], cwd=repo_root, check=True)


@app.command()
def land(
    force: bool = typer.Option(
        False, "-f", "--force", help="Bypass PR existence warning"
    ),
    no_pr: bool = typer.Option(
        False, "-n", "--no-pr", help="Bypass PR workflow check"
    ),
    base: str | None = typer.Option(
        None, "-b", "--base", help="Override base branch (default: repo default)"
    ),
    require_clean_design: bool = typer.Option(
        False,
        "--require-clean-design",
        help="Fail if design artifacts are present instead of removing them",
    ),
) -> None:
    """Land this branch locally using worktrunk."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not shutil.which("wt"):
        typer.echo("Error: 'wt' CLI not found. Run: lf meta install", err=True)
        raise typer.Exit(1)

    main_repo = find_main_repo(repo_root) or repo_root
    config = load_config(main_repo)
    pr_enabled = config.pr if config else False

    pr_exists = _get_pr_status(repo_root)
    if pr_exists is True and not force:
        if pr_enabled:
            typer.echo("Warning: PR exists, use 'lf pr land'", err=True)
        else:
            typer.echo("Warning: PR exists, use 'lf pr land' or --force", err=True)
        raise typer.Exit(1)

    if pr_exists is False and pr_enabled and not no_pr:
        typer.echo(
            "Warning: PR workflow enabled, use 'lf pr create' first or --no-pr",
            err=True,
        )
        raise typer.Exit(1)

    if pr_exists is None and pr_enabled and not no_pr:
        typer.echo(
            "Warning: Could not verify PR status (gh not available). "
            "Use --no-pr to bypass.",
            err=True,
        )
        raise typer.Exit(1)

    _ensure_clean(repo_root)

    branch = get_current_branch(repo_root)
    if not branch:
        typer.echo("Error: Detached HEAD", err=True)
        raise typer.Exit(1)

    base_branch = base or _get_default_branch(main_repo)
    if branch == base_branch:
        typer.echo(f"Error: Cannot land {branch} onto itself", err=True)
        raise typer.Exit(1)

    subprocess.run(["git", "fetch", "origin", base_branch], cwd=repo_root, check=False)

    if require_clean_design:
        if has_design_artifacts(repo_root):
            typer.echo(
                "Error: design artifacts present. Remove .design contents before landing.",
                err=True,
            )
            raise typer.Exit(1)
    else:
        _clear_design_artifacts(repo_root)

    base_ref = _resolve_base_ref(repo_root, base_branch)
    diff = _get_diff(repo_root, base_ref)
    if not diff.strip():
        typer.echo("Error: No changes to land", err=True)
        raise typer.Exit(1)

    message = generate_commit_message_from_diff(repo_root, diff)
    commit_msg = message.title
    if message.body:
        commit_msg += f"\n\n{message.body}"

    _squash_commits(repo_root, base_ref, commit_msg)

    was_in_worktree = repo_root != main_repo

    cmd = ["wt", "merge", "--no-squash"]
    if base:
        cmd.append(base_branch)
    subprocess.run(cmd, cwd=repo_root, check=True)

    # Output main repo path for shell cd integration when we removed a worktree
    if was_in_worktree:
        typer.echo(str(main_repo))

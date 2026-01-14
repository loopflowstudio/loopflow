"""lfpr: Pull request and landing operations CLI."""

import json
import shutil
import subprocess
import sys
from pathlib import Path

import typer

from loopflow.config import load_config
from loopflow.context import find_worktree_root
from loopflow.design import clear_design_artifacts, has_design_artifacts
from loopflow.git import GitError, find_main_repo, get_current_branch, has_upstream, open_pr, update_pr
from loopflow.llm_http import generate_commit_message, generate_commit_message_from_diff, generate_pr_message
from loopflow.worktrees import get_path, remove

app = typer.Typer(help="Pull request operations")


def _add_commit_push(repo_root: Path, push: bool = True) -> bool:
    """Add, commit (with generated message), and optionally push. Returns True if committed."""
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if not result.stdout.strip():
        if push:
            typer.echo("Pushing...")
            subprocess.run(["git", "push"], cwd=repo_root, check=True)
        return False

    typer.echo("Staging changes...")
    subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)

    typer.echo("Generating commit message...")
    message = generate_commit_message(repo_root)
    commit_msg = message.title
    if message.body:
        commit_msg += f"\n\n{message.body}"

    typer.echo(f"Committing: {message.title}")
    subprocess.run(["git", "commit", "-m", commit_msg], cwd=repo_root, check=True)

    if push:
        typer.echo("Pushing...")
        subprocess.run(["git", "push"], cwd=repo_root, check=True)

    return True


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
def create(
    add: bool = typer.Option(False, "-a", "--add", help="Add, commit, and push changes first"),
) -> None:
    """Create a GitHub PR for this branch with generated title/body."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not shutil.which("gh"):
        typer.echo("Error: 'gh' CLI not found. Install with: brew install gh", err=True)
        raise typer.Exit(1)

    if add:
        _add_commit_push(repo_root)

    typer.echo("Generating PR title and body...")
    message = generate_pr_message(repo_root)

    typer.echo(f"\n{message.title}\n")
    typer.echo(message.body)
    typer.echo("")

    try:
        pr_url = open_pr(repo_root, title=message.title, body=message.body)
    except GitError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)
    typer.echo(pr_url)
    subprocess.run(["open", pr_url])


@app.command()
def view() -> None:
    """Open PR in browser (or show status if no PR)."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not shutil.which("gh"):
        typer.echo("Error: 'gh' CLI not found. Install with: brew install gh", err=True)
        raise typer.Exit(1)

    result = subprocess.run(
        ["gh", "pr", "view", "--json", "url", "-q", ".url"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )

    if result.returncode == 0 and result.stdout.strip():
        url = result.stdout.strip()
        subprocess.run(["open", url])
        typer.echo(f"Opened: {url}")
    else:
        typer.echo("No PR for current branch")
        typer.echo("Create one with: lfpr create")


@app.command()
def update(
    add: bool = typer.Option(False, "-a", "--add", help="Add, commit, and push changes first"),
) -> None:
    """Update existing PR title/body with regenerated message."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not shutil.which("gh"):
        typer.echo("Error: 'gh' CLI not found. Install with: brew install gh", err=True)
        raise typer.Exit(1)

    if add:
        _add_commit_push(repo_root)

    typer.echo("Generating PR title and body...")
    message = generate_pr_message(repo_root)

    typer.echo(f"\n{message.title}\n")
    typer.echo(message.body)
    typer.echo("")

    try:
        pr_url = update_pr(repo_root, title=message.title, body=message.body)
    except GitError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)
    typer.echo(f"Updated: {pr_url}")


@app.command()
def land(
    add: bool = typer.Option(False, "-a", "--add", help="Add, commit, and push changes first"),
    worktree: str = typer.Option(None, "-w", "--worktree", help="Target specific worktree by name"),
    local: bool = typer.Option(False, "-l", "--local", help="Use local landing (worktrunk) instead of PR"),
    force: bool = typer.Option(False, "-f", "--force", help="Bypass PR existence warning (local mode)"),
    no_pr: bool = typer.Option(False, "-n", "--no-pr", help="Bypass PR workflow check (local mode)"),
    base: str | None = typer.Option(None, "-b", "--base", help="Override base branch"),
    require_clean_design: bool = typer.Option(
        False,
        "--require-clean-design",
        help="Fail if design artifacts are present instead of removing them",
    ),
) -> None:
    """Land this branch: squash-merge to base branch and clean up.

    By default uses PR-based landing (requires PR to exist).
    Use --local for worktrunk-based local landing.
    """
    if local:
        _land_local(worktree, force, no_pr, base, require_clean_design)
    else:
        _land_pr(add, worktree, require_clean_design)


def _land_pr(
    add: bool,
    worktree: str | None,
    require_clean_design: bool,
) -> None:
    """PR-based landing workflow."""
    if worktree:
        main_repo = find_main_repo()
        if not main_repo:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)
        repo_root = get_path(main_repo, worktree)
        if not repo_root.exists():
            typer.echo(f"Error: Worktree '{worktree}' not found", err=True)
            raise typer.Exit(1)
    else:
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)
        main_repo = find_main_repo(repo_root)
        if not main_repo:
            typer.echo("Error: Could not find main repository", err=True)
            raise typer.Exit(1)

    if not shutil.which("gh"):
        typer.echo("Error: 'gh' CLI not found. Install with: brew install gh", err=True)
        raise typer.Exit(1)

    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    branch = result.stdout.strip()

    if not branch:
        typer.echo("Error: Detached HEAD", err=True)
        raise typer.Exit(1)

    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    has_changes = bool(result.stdout.strip())

    if has_changes:
        if add:
            _add_commit_push(repo_root, push=False)
        else:
            typer.echo("Error: Uncommitted changes. Use --add or commit manually.", err=True)
            raise typer.Exit(1)

    if require_clean_design and has_design_artifacts(repo_root):
        typer.echo(
            "Error: design artifacts present. Remove .design contents before landing.",
            err=True,
        )
        raise typer.Exit(1)

    result = subprocess.run(
        ["git", "rev-parse", "@{u}"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    has_upstream_branch = result.returncode == 0

    if has_upstream_branch:
        result = subprocess.run(
            ["git", "rev-list", "@{u}..HEAD", "--count"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        unpushed = int(result.stdout.strip()) if result.returncode == 0 else 0

        if unpushed > 0:
            if add:
                typer.echo("Pushing to origin...")
                subprocess.run(["git", "push"], cwd=repo_root, check=True)
            else:
                typer.echo("Error: Unpushed commits. Use --add or push manually.", err=True)
                raise typer.Exit(1)
    else:
        if add:
            typer.echo("Pushing to origin...")
            subprocess.run(["git", "push", "-u", "origin", branch], cwd=repo_root, check=True)
        else:
            typer.echo("Error: Branch not pushed. Use --add or push manually.", err=True)
            raise typer.Exit(1)

    result = subprocess.run(
        ["gh", "pr", "view", "--json", "title,body,baseRefName"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        typer.echo("Error: No PR found. Run 'lfpr create' first.", err=True)
        raise typer.Exit(1)

    pr_data = json.loads(result.stdout)
    title = pr_data.get("title", "").strip()
    body = pr_data.get("body", "").strip()
    base_branch = pr_data.get("baseRefName", "main").strip()

    if not title:
        typer.echo("Error: PR has no title", err=True)
        raise typer.Exit(1)

    if branch == base_branch:
        typer.echo(f"Error: Cannot land {branch} onto itself", err=True)
        raise typer.Exit(1)

    commit_msg = title
    if body:
        commit_msg += f"\n\n{body}"

    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        typer.echo("Error: Main repo has uncommitted changes", err=True)
        raise typer.Exit(1)

    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    original_branch = result.stdout.strip()

    typer.echo(f"Checking out {base_branch}...")
    subprocess.run(["git", "fetch", "origin", base_branch], cwd=main_repo, check=True)
    subprocess.run(["git", "checkout", base_branch], cwd=main_repo, check=True)
    subprocess.run(["git", "reset", "--hard", f"origin/{base_branch}"], cwd=main_repo, check=True)

    subprocess.run(["git", "fetch", "origin", branch], cwd=main_repo, check=True)
    result = subprocess.run(
        ["git", "merge", "--squash", f"origin/{branch}"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        typer.echo(f"Error: Merge failed. Resolve conflicts manually.\n{result.stderr}", err=True)
        raise typer.Exit(1)

    if clear_design_artifacts(main_repo):
        design_dir = main_repo / ".design"
        if design_dir.exists():
            subprocess.run(["git", "add", "-A", str(design_dir)], cwd=main_repo, check=True)
        typer.echo("Removed .design contents")

    result = subprocess.run(
        ["git", "diff", "--cached", "--quiet"],
        cwd=main_repo,
    )
    if result.returncode == 0:
        typer.echo(f"Nothing to land - {branch} has no changes relative to {base_branch}.", err=True)
        if original_branch:
            subprocess.run(["git", "checkout", original_branch], cwd=main_repo, capture_output=True)
        raise typer.Exit(1)

    subprocess.run(["git", "commit", "-m", commit_msg], cwd=main_repo, check=True)
    subprocess.run(["git", "push"], cwd=main_repo, check=True)

    subprocess.run(
        ["git", "push", "origin", "--delete", branch],
        cwd=main_repo,
        capture_output=True,
    )

    was_in_worktree = repo_root != main_repo
    if was_in_worktree:
        remove(main_repo, branch)
    else:
        if original_branch and original_branch != branch:
            subprocess.run(["git", "checkout", original_branch], cwd=main_repo, capture_output=True)
        subprocess.run(["git", "branch", "-D", branch], cwd=main_repo, check=True)

    typer.echo(f"Landed {branch} onto {base_branch} and pushed.")

    if was_in_worktree:
        typer.echo(str(main_repo))


def _land_local(
    worktree: str | None,
    force: bool,
    no_pr: bool,
    base: str | None,
    require_clean_design: bool,
) -> None:
    """Local landing workflow using worktrunk."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not shutil.which("wt"):
        typer.echo("Error: 'wt' CLI not found. Run: lfops install", err=True)
        raise typer.Exit(1)

    main_repo = find_main_repo(repo_root) or repo_root
    config = load_config(main_repo)
    pr_enabled = config.pr if config else False

    pr_exists = _get_pr_status(repo_root)
    if pr_exists is True and not force:
        typer.echo("Warning: PR exists, use 'lfpr land' (without --local) or --force", err=True)
        raise typer.Exit(1)

    if pr_exists is False and pr_enabled and not no_pr:
        typer.echo(
            "Warning: PR workflow enabled, use 'lfpr create' first or --no-pr",
            err=True,
        )
        raise typer.Exit(1)

    if pr_exists is None and pr_enabled and not no_pr:
        typer.echo(
            "Warning: Could not verify PR status (gh not available). Use --no-pr to bypass.",
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
        clear_design_artifacts(repo_root)

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

    subprocess.run(
        ["git", "push", "origin", "--delete", branch],
        cwd=main_repo,
        capture_output=True,
    )

    if was_in_worktree:
        typer.echo(str(main_repo))


@app.command()
def commit(
    push: bool = typer.Option(False, "-p", "--push", help="Push after committing"),
    add: bool = typer.Option(True, "-a/-A", "--add/--no-add", help="Stage all changes before committing"),
) -> None:
    """Generate commit message from diff and commit."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if not result.stdout.strip():
        typer.echo("Nothing to commit", err=True)
        raise typer.Exit(0)

    if add:
        subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)

    staged = subprocess.run(
        ["git", "diff", "--cached", "--quiet"],
        cwd=repo_root,
    )
    if staged.returncode == 0:
        typer.echo("Nothing staged to commit", err=True)
        raise typer.Exit(0)

    typer.echo("Generating commit message...")
    try:
        message = generate_commit_message(repo_root)
    except Exception as e:
        typer.echo(f"Error generating commit message: {e}", err=True)
        raise typer.Exit(1)

    commit_msg = message.title
    if message.body:
        commit_msg += f"\n\n{message.body}"

    result = subprocess.run(
        ["git", "commit", "-m", commit_msg],
        cwd=repo_root,
    )
    if result.returncode != 0:
        typer.echo("Commit failed", err=True)
        raise typer.Exit(1)

    typer.echo(f"Committed: {message.title}")

    if push:
        if has_upstream(repo_root):
            result = subprocess.run(["git", "push"], cwd=repo_root)
            if result.returncode == 0:
                typer.echo("Pushed to origin")
            else:
                typer.echo("Push failed", err=True)
                raise typer.Exit(1)
        else:
            typer.echo("No upstream branch, skipping push", err=True)


def main() -> None:
    """Entry point for lfpr command."""
    if len(sys.argv) == 1:
        sys.argv.append("view")
    app()


if __name__ == "__main__":
    main()

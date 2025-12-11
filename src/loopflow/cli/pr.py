"""Pull request workflow commands."""

import json
import shutil
import subprocess

import typer

from loopflow.context import find_worktree_root
from loopflow.git import GitError, find_main_repo, open_pr, update_pr
from loopflow.llm_http import generate_commit_message, generate_pr_message

app = typer.Typer(help="Pull request workflow.")


def _add_commit_push(repo_root, push: bool = True) -> bool:
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


@app.command()
def create(
    add: bool = typer.Option(False, "-a", "--add", help="Add, commit, and push changes first"),
):
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
def update(
    add: bool = typer.Option(False, "-a", "--add", help="Add, commit, and push changes first"),
):
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
):
    """Land this branch: squash-merge to main and clean up.

    Requires a PR with a title. Commit message is PR title + body.
    Branch must be clean and pushed (use --add to auto-commit first).
    """
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    # Get main repo (different from repo_root when in a worktree)
    main_repo = find_main_repo(repo_root)
    if not main_repo:
        typer.echo("Error: Could not find main repository", err=True)
        raise typer.Exit(1)

    if not shutil.which("gh"):
        typer.echo("Error: 'gh' CLI not found. Install with: brew install gh", err=True)
        raise typer.Exit(1)

    # Get current branch
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    branch = result.stdout.strip()

    if not branch or branch == "main":
        typer.echo("Error: Already on main (or detached HEAD)", err=True)
        raise typer.Exit(1)

    # Check for uncommitted changes
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

    # Check if pushed to origin
    result = subprocess.run(
        ["git", "rev-parse", "@{u}"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    has_upstream = result.returncode == 0

    if has_upstream:
        # Check if local is ahead of remote
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

    # Get commit message from PR
    result = subprocess.run(
        ["gh", "pr", "view", "--json", "title,body"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        typer.echo("Error: No PR found. Run 'lf pr create' first.", err=True)
        raise typer.Exit(1)

    pr_data = json.loads(result.stdout)
    title = pr_data.get("title", "").strip()
    body = pr_data.get("body", "").strip()

    if not title:
        typer.echo("Error: PR has no title", err=True)
        raise typer.Exit(1)

    commit_msg = title
    if body:
        commit_msg += f"\n\n{body}"

    # Land it (operations happen in main repo where main is checked out)
    subprocess.run(["git", "fetch", "origin", branch], cwd=main_repo, check=True)
    subprocess.run(["git", "merge", "--squash", f"origin/{branch}"], cwd=main_repo, check=True)
    subprocess.run(["git", "commit", "-m", commit_msg], cwd=main_repo, check=True)
    subprocess.run(["git", "push"], cwd=main_repo, check=True)

    # Clean up: remove worktree and branch
    from loopflow.git import remove_worktree
    if repo_root != main_repo:
        remove_worktree(main_repo, branch)
    else:
        subprocess.run(["git", "branch", "-D", branch], cwd=main_repo, check=True)

    typer.echo(f"Landed {branch} to main and pushed.")

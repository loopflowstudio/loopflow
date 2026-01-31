"""Land command for merging branches to main."""

import json
import shutil
import subprocess
from pathlib import Path

import typer

from loopflow.lf.config import load_config, parse_model
from loopflow.lf.context import find_worktree_root
from loopflow.lf.design import clear_design_artifacts
from loopflow.lf.git import (
    GitError,
    ensure_ready_pr,
    find_main_repo,
    get_current_branch,
    is_draft_pr,
    open_pr,
)
from loopflow.lf.launcher import get_runner
from loopflow.lf.messages import generate_commit_message_from_diff, generate_pr_message
from loopflow.lf.ops.git import push as git_push
from loopflow.lf.ops.git import rebase as git_rebase
from loopflow.lf.worktrees import get_path
from loopflow.lf.ops._helpers import (
    add_commit_push,
    get_default_branch,
    get_diff,
    remove_worktree,
    resolve_base_ref,
    run_lint,
)


def _resolve_repos(worktree: str | None, strict: bool) -> tuple[Path, Path]:
    """Resolve repo_root and main_repo from worktree param or cwd.

    Also handles uncommitted changes: commits them unless strict mode.
    Returns (repo_root, main_repo).
    """
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
        main_repo = find_main_repo(repo_root) or repo_root

    # Handle uncommitted changes
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        if strict:
            typer.echo(
                "Error: Uncommitted changes (use without --strict to auto-commit)",
                err=True,
            )
            raise typer.Exit(1)
        add_commit_push(repo_root, push=False)

    return repo_root, main_repo


def _clear_design_and_push(repo_root: Path) -> bool:
    """Delete scratch/* contents, commit, push. Returns True if changes made."""
    design_dir = repo_root / "scratch"
    if not design_dir.exists():
        return False

    files = list(design_dir.glob("*"))
    if not files:
        return False

    for f in files:
        if f.is_file():
            f.unlink()
        else:
            shutil.rmtree(f)

    subprocess.run(["git", "add", "-A", str(design_dir)], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", "clear scratch/"], cwd=repo_root, check=True)
    subprocess.run(["git", "push"], cwd=repo_root, check=True)
    return True


def _auto_merge_not_allowed(message: str) -> bool:
    lowered = message.lower()
    return "auto merge is not allowed" in lowered or "enablepullrequestautomerge" in lowered


def _squash_commits(repo_root: Path, base_ref: str, commit_msg: str) -> None:
    original_head = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()

    subprocess.run(["git", "reset", "--soft", base_ref], cwd=repo_root, check=True)
    design_dir = repo_root / "scratch"
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


def _rebase_onto_main(repo_root: Path, base_branch: str) -> bool:
    """Rebase branch onto base_branch. Returns True if successful.

    If conflicts occur, launches the rebase task assistant to resolve them.
    Handles force-push after rebase if the branch has an upstream.
    """
    from loopflow.lf.context import gather_step
    from loopflow.lf.ops.git import GitError

    # Fetch latest main
    subprocess.run(["git", "fetch", "origin", base_branch], cwd=repo_root, check=False)

    # Check if rebase is needed
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", f"origin/{base_branch}", "HEAD"],
        cwd=repo_root,
        capture_output=True,
    )
    if result.returncode == 0:
        # Already up-to-date
        return True

    typer.echo(f"Rebasing onto {base_branch}...")
    try:
        rebase_result = git_rebase(repo_root, f"origin/{base_branch}")
    except GitError as e:
        typer.echo(f"Error: Rebase failed: {e}", err=True)
        return False

    if not rebase_result.success:
        # Conflicts - hand off to assistant (git_rebase already aborted)
        typer.echo("Conflicts detected, launching rebase assistant...")

        # Get rebase prompt (custom or built-in)
        config = load_config(repo_root)
        step = gather_step(repo_root, "rebase", config=config)
        if not step:
            typer.echo("Error: No rebase step found", err=True)
            return False

        # Run agent with the rebase step
        agent_model = step.config.model or (config.agent_model if config else "claude:opus")
        backend, model_variant = parse_model(agent_model)
        runner = get_runner(backend)
        agent_result = runner.launch(
            step.content,
            auto=True,
            stream=True,
            skip_permissions=True,
            model_variant=model_variant,
            cwd=repo_root,
        )
        if agent_result.exit_code != 0:
            typer.echo("Rebase assistant failed", err=True)
            return False

        # Verify rebase completed (branch should now be ahead of main)
        result = subprocess.run(
            ["git", "merge-base", "--is-ancestor", f"origin/{base_branch}", "HEAD"],
            cwd=repo_root,
            capture_output=True,
        )
        if result.returncode != 0:
            typer.echo("Error: Rebase did not complete successfully", err=True)
            return False

        return True

    # Force-push if branch has upstream (rebase rewrites history)
    result = subprocess.run(
        ["git", "rev-parse", "@{u}"],
        cwd=repo_root,
        capture_output=True,
    )
    if result.returncode == 0:
        typer.echo("Force-pushing rebased branch...")
        try:
            git_push(repo_root, force_with_lease=True)
        except GitError as e:
            typer.echo(f"Error: Force-push failed after rebase: {e}", err=True)
            return False

    return True


def _land_pr(strict: bool, worktree: str | None, create_pr: bool = False) -> None:
    """Land via GitHub PR merge."""
    repo_root, main_repo = _resolve_repos(worktree, strict)

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

    # Rebase onto base branch before pushing
    base_branch = get_default_branch(main_repo)
    if not _rebase_onto_main(repo_root, base_branch):
        raise typer.Exit(1)

    # Ensure branch is pushed
    result = subprocess.run(
        ["git", "rev-parse", "@{u}"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    has_upstream_branch = result.returncode == 0

    if has_upstream_branch:
        # Check if upstream matches the local branch name
        result = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "@{u}"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        upstream_branch = result.stdout.strip() if result.returncode == 0 else ""
        upstream_name = (
            upstream_branch.split("/", 1)[-1] if "/" in upstream_branch else upstream_branch
        )
        upstream_matches = upstream_name == branch

        result = subprocess.run(
            ["git", "rev-list", "@{u}..HEAD", "--count"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        unpushed = int(result.stdout.strip()) if result.returncode == 0 else 0
        # Check if branches have diverged (remote has commits not in local)
        result = subprocess.run(
            ["git", "rev-list", "HEAD..@{u}", "--count"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        remote_ahead = int(result.stdout.strip()) if result.returncode == 0 else 0
        if unpushed > 0 or not upstream_matches:
            if strict:
                typer.echo(
                    "Error: Unpushed commits (use without --strict to auto-push)",
                    err=True,
                )
                raise typer.Exit(1)
            if remote_ahead > 0 and upstream_matches:
                # Branches diverged (e.g., after rebase) - force push safely
                typer.echo("Branches diverged, force-pushing with lease...")
                subprocess.run(["git", "push", "--force-with-lease"], cwd=repo_root, check=True)
            else:
                # Push to same-named branch on origin (handles mismatched upstream)
                typer.echo("Pushing to origin...")
                subprocess.run(["git", "push", "-u", "origin", branch], cwd=repo_root, check=True)
    else:
        if strict:
            typer.echo("Error: Branch not pushed (use without --strict to auto-push)", err=True)
            raise typer.Exit(1)
        typer.echo("Pushing to origin...")
        subprocess.run(["git", "push", "-u", "origin", branch], cwd=repo_root, check=True)

    # Get PR info (or create PR if --create-pr)
    # Use --state open to avoid finding old closed/merged PRs with the same branch name
    result = subprocess.run(
        ["gh", "pr", "view", "--json", "number,title,body,baseRefName,state"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    pr_data = None
    if result.returncode == 0:
        pr_data = json.loads(result.stdout)
        # Ignore closed/merged PRs - we need an open one
        if pr_data.get("state", "").upper() != "OPEN":
            pr_data = None

    if pr_data is None:
        if create_pr:
            typer.echo("Creating PR...")
            message = generate_pr_message(repo_root)
            try:
                pr_url = open_pr(repo_root, title=message.title, body=message.body)
            except GitError as e:
                typer.echo(f"Error creating PR: {e}", err=True)
                raise typer.Exit(1)
            typer.echo(f"Created: {pr_url}")
            subprocess.run(["open", pr_url])
            # Re-fetch to get the PR number
            result = subprocess.run(
                ["gh", "pr", "view", "--json", "number,title,body,baseRefName"],
                cwd=repo_root,
                capture_output=True,
                text=True,
            )
            if result.returncode != 0:
                typer.echo("Error: Could not get PR info after creation", err=True)
                raise typer.Exit(1)
            pr_data = json.loads(result.stdout)
            pr_number = pr_data.get("number")
            title = message.title
            body = message.body
            base_branch = get_default_branch(main_repo)
        else:
            typer.echo(
                "Error: No open PR found. Run 'lf ops pr' first, or use --local or --create-pr.",
                err=True,
            )
            raise typer.Exit(1)
    else:
        pr_number = pr_data.get("number")
        base_branch = pr_data.get("baseRefName", "main").strip()
        # Always regenerate PR title/body to reflect latest changes
        typer.echo("Refreshing PR...")
        message = generate_pr_message(repo_root)
        title = message.title
        body = message.body
        subprocess.run(
            ["gh", "pr", "edit", str(pr_number), "--title", title, "--body", body],
            cwd=repo_root,
            check=True,
        )

    if not title:
        typer.echo("Error: PR has no title", err=True)
        raise typer.Exit(1)

    if branch == base_branch:
        typer.echo(f"Error: Cannot land {branch} onto itself", err=True)
        raise typer.Exit(1)

    # Clear scratch/ before merge so it never touches main
    # Then update PR so it points to the new HEAD
    if _clear_design_and_push(repo_root):
        typer.echo("Cleared scratch/")
        subprocess.run(
            ["gh", "pr", "edit", str(pr_number), "--title", title, "--body", body],
            cwd=repo_root,
            capture_output=True,
        )

    if is_draft_pr(repo_root, pr_number):
        typer.echo("Marking PR as ready for review...")
        if not ensure_ready_pr(repo_root, pr_number):
            typer.echo("Error: Failed to mark PR as ready", err=True)
            raise typer.Exit(1)

    # Enable auto-merge on the PR
    # With merge queue enabled, this queues the PR for merge after CI passes
    typer.echo(f"Enabling auto-merge for PR #{pr_number}: {title}")
    merge_cmd = [
        "gh",
        "pr",
        "merge",
        str(pr_number),
        "--squash",
        "--auto",
        "--subject",
        title,
    ]
    if body:
        merge_cmd.extend(["--body", body])
    result = subprocess.run(merge_cmd, cwd=repo_root, capture_output=True, text=True)
    auto_merge_enabled = True
    if result.returncode != 0:
        error_msg = result.stderr.strip() or result.stdout.strip() or "auto-merge failed"
        if _auto_merge_not_allowed(error_msg):
            auto_merge_enabled = False
            msg = "Auto-merge is disabled for this repo. "
            msg += "Enable it in repo settings or merge manually after CI passes."
            typer.echo(msg, err=True)
        else:
            typer.echo(f"Error: {error_msg}", err=True)
            raise typer.Exit(1)

    # Get PR URL and open in browser
    result = subprocess.run(
        ["gh", "pr", "view", str(pr_number), "--json", "url", "-q", ".url"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    pr_url = result.stdout.strip() if result.returncode == 0 else None
    if pr_url:
        subprocess.run(["open", pr_url])

    was_in_worktree = repo_root != main_repo
    if auto_merge_enabled:
        typer.echo(f"PR #{pr_number} queued for merge. Will merge when CI passes.")
    else:
        typer.echo(f"PR #{pr_number} not queued for auto-merge.")
        typer.echo(
            "Enable auto-merge in repo settings or run `gh pr merge --squash` after CI passes."
        )
    typer.echo("Run 'lf ops wt prune' after merge completes.")

    if was_in_worktree:
        typer.echo(str(main_repo))


def _land_local(strict: bool, worktree: str | None) -> None:
    """Land locally without PR (squash-merge + push)."""
    repo_root, main_repo = _resolve_repos(worktree, strict)

    branch = get_current_branch(repo_root)
    if not branch:
        typer.echo("Error: Detached HEAD", err=True)
        raise typer.Exit(1)

    base_branch = get_default_branch(main_repo)
    if branch == base_branch:
        typer.echo(f"Error: Cannot land {branch} onto itself", err=True)
        raise typer.Exit(1)

    # Rebase onto base branch before merging
    if not _rebase_onto_main(repo_root, base_branch):
        raise typer.Exit(1)

    # Check for changes
    base_ref = resolve_base_ref(repo_root, base_branch)
    diff = get_diff(repo_root, base_ref)
    if not diff.strip():
        typer.echo("Error: No changes to land", err=True)
        raise typer.Exit(1)

    # Generate commit message
    typer.echo("Generating commit message...")
    message = generate_commit_message_from_diff(repo_root, diff)
    commit_msg = message.title
    if message.body:
        commit_msg += f"\n\n{message.body}"

    # Squash commits on the branch
    _squash_commits(repo_root, base_ref, commit_msg)

    # Check main repo is clean
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    tracked_changes = [
        line for line in result.stdout.strip().split("\n") if line and not line.startswith("??")
    ]
    if tracked_changes:
        typer.echo("Error: Main repo has uncommitted changes", err=True)
        raise typer.Exit(1)

    # Checkout and reset main to origin
    typer.echo(f"Checking out {base_branch}...")
    result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "HEAD"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    current_branch = result.stdout.strip()

    if current_branch != base_branch:
        result = subprocess.run(
            ["git", "checkout", base_branch],
            cwd=main_repo,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            typer.echo(f"Error: Could not checkout {base_branch} in main repo", err=True)
            typer.echo(f"  {result.stderr.strip()}", err=True)
            raise typer.Exit(1)

    subprocess.run(["git", "reset", "--hard", f"origin/{base_branch}"], cwd=main_repo, check=True)

    # Fetch and merge the branch
    subprocess.run(["git", "fetch", "origin", branch], cwd=main_repo, check=False)

    # Try to merge from origin first (if pushed), otherwise from local worktree
    result = subprocess.run(
        ["git", "rev-parse", "--verify", f"origin/{branch}"],
        cwd=main_repo,
        capture_output=True,
    )
    if result.returncode == 0:
        merge_ref = f"origin/{branch}"
    else:
        # Branch not pushed, merge from worktree path
        merge_ref = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()

    result = subprocess.run(
        ["git", "merge", "--squash", merge_ref],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        typer.echo(f"Error: Merge failed.\n{result.stderr}", err=True)
        raise typer.Exit(1)

    # Clear scratch/ artifacts
    if clear_design_artifacts(main_repo):
        design_dir = main_repo / "scratch"
        if design_dir.exists():
            subprocess.run(["git", "add", "-A", str(design_dir)], cwd=main_repo, check=True)
        typer.echo("Removed scratch/ contents")

    # Check there's something to commit
    result = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=main_repo)
    if result.returncode == 0:
        typer.echo(
            f"Nothing to land - {branch} has no changes relative to {base_branch}.",
            err=True,
        )
        raise typer.Exit(1)

    # Commit and push
    typer.echo(f"Committing: {message.title}")
    subprocess.run(["git", "commit", "-m", commit_msg], cwd=main_repo, check=True)
    subprocess.run(["git", "push"], cwd=main_repo, check=True)

    # Delete remote branch if it exists
    subprocess.run(
        ["git", "push", "origin", "--delete", branch],
        cwd=main_repo,
        capture_output=True,
    )

    # Clean up worktree/branch (best-effort after push)
    was_in_worktree = repo_root != main_repo
    if was_in_worktree:
        try:
            remove_worktree(main_repo, branch, repo_root, base_branch)
        except Exception:
            typer.echo("Warning: Could not remove worktree. Run manually:", err=True)
            typer.echo("  lf ops wt prune", err=True)
    else:
        subprocess.run(["git", "branch", "-D", branch], cwd=main_repo, capture_output=True)

    typer.echo(f"Landed {branch} onto {base_branch}.")

    if was_in_worktree:
        typer.echo(str(main_repo))


def _land_squash_loop_main(strict: bool, worktree: str | None) -> None:
    """Squash-merge loop-main to origin/main via PR."""
    repo_root, _ = _resolve_repos(worktree, strict)

    if not shutil.which("gh"):
        typer.echo("Error: 'gh' CLI not found. Install with: brew install gh", err=True)
        raise typer.Exit(1)

    branch = get_current_branch(repo_root)
    if not branch:
        typer.echo("Error: Detached HEAD", err=True)
        raise typer.Exit(1)

    # Verify this is a loop-main branch
    if not branch.endswith("-main"):
        typer.echo(f"Error: --squash is for loop-main branches (got '{branch}')", err=True)
        typer.echo("Run this from a loop-main worktree like 'agent-name-main'", err=True)
        raise typer.Exit(1)

    # Ensure branch is pushed
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
            if strict:
                typer.echo(
                    "Error: Unpushed commits (use without --strict to auto-push)",
                    err=True,
                )
                raise typer.Exit(1)
            typer.echo("Pushing to origin...")
            subprocess.run(["git", "push"], cwd=repo_root, check=True)
    else:
        if strict:
            typer.echo("Error: Branch not pushed (use without --strict to auto-push)", err=True)
            raise typer.Exit(1)
        typer.echo("Pushing to origin...")
        subprocess.run(["git", "push", "-u", "origin", branch], cwd=repo_root, check=True)

    # Generate PR message from full diff against main
    typer.echo("Generating PR message...")
    message = generate_pr_message(repo_root)

    # Create PR from loop-main to main
    typer.echo(f"Creating PR: {branch} → main")
    cmd = [
        "gh",
        "pr",
        "create",
        "--base",
        "main",
        "--head",
        branch,
        "--title",
        message.title,
        "--body",
        message.body,
    ]
    result = subprocess.run(cmd, cwd=repo_root, capture_output=True, text=True)

    if result.returncode != 0:
        if "already exists" in result.stderr.lower():
            typer.echo("PR already exists, updating...")
            subprocess.run(
                ["gh", "pr", "edit", "--title", message.title, "--body", message.body],
                cwd=repo_root,
                capture_output=True,
            )
        else:
            typer.echo(f"Error creating PR: {result.stderr}", err=True)
            raise typer.Exit(1)

    # Get PR info
    result = subprocess.run(
        ["gh", "pr", "view", "--json", "number,url"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        typer.echo("Error: Could not get PR info", err=True)
        raise typer.Exit(1)

    pr_data = json.loads(result.stdout)
    pr_number = pr_data.get("number")
    pr_url = pr_data.get("url")

    typer.echo(f"PR #{pr_number}: {pr_url}")
    typer.echo("")
    typer.echo(f"{message.title}")
    if message.body:
        typer.echo("")
        typer.echo(message.body[:500] + "..." if len(message.body) > 500 else message.body)

    subprocess.run(["open", pr_url])


def register_commands(app: typer.Typer) -> None:
    """Register land command on the app."""

    @app.command()
    def land(
        worktree: str = typer.Option(None, "-w", "--worktree", help="Target worktree by name"),
        local: bool = typer.Option(
            None, "-l", "--local/--gh", help="Local merge (no PR) vs GitHub PR merge"
        ),
        create_pr: bool = typer.Option(
            False, "-c", "--create-pr", help="Create PR and merge in one step"
        ),
        strict: bool = typer.Option(
            False, "-s", "--strict", help="Error if uncommitted/unpushed changes exist"
        ),
        squash: bool = typer.Option(
            False, "--squash", help="Squash-merge loop-main to origin/main"
        ),
        lint: bool = typer.Option(True, "--lint/--no-lint", help="Run lint before landing"),
    ) -> None:
        """Squash-merge branch to main and clean up.

        Automatically rebases onto main before merging to ensure clean merge.
        By default, stages, commits, and pushes any pending changes before landing.
        Use --strict to require clean state (error if uncommitted/unpushed).

        Default: uses gh pr merge (requires PR via lf ops pr).
        With --local: local merge + push (no PR needed).
        With --create-pr: create PR and immediately merge.
        With --squash: squash-merge entire loop-main branch to main (for agents).
        Config: set `land: local` in .lf/config.yaml to default to --local.
        """
        main_repo = find_main_repo()
        config = load_config(main_repo) if main_repo else None

        if lint:
            lint_repo = main_repo or find_worktree_root()
            if lint_repo and not run_lint(lint_repo):
                typer.echo("Lint failed, aborting land", err=True)
                raise typer.Exit(1)

        if squash:
            _land_squash_loop_main(strict, worktree)
            return

        use_local = local if local is not None else (config and config.land == "local")

        if use_local:
            _land_local(strict, worktree)
        else:
            _land_pr(strict, worktree, create_pr=create_pr)

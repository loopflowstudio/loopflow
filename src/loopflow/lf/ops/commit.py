"""Commit command for auto-generating commit messages."""

import subprocess

import typer

from loopflow.lf.config import load_config, parse_model
from loopflow.lf.context import (
    ContextConfig,
    find_worktree_root,
    format_prompt,
    gather_prompt_components,
    gather_step,
)
from loopflow.lf.git import ensure_draft_pr, has_upstream
from loopflow.lf.launcher import get_runner
from loopflow.lf.ops._helpers import run_lint


def register_commands(app: typer.Typer) -> None:
    """Register commit command on the app."""

    @app.command()
    def commit(
        push: bool = typer.Option(False, "-p", "--push", help="Push after committing"),
        add: bool = typer.Option(True, "-a/-A", "--add/--no-add", help="Stage changes first"),
        lint: bool = typer.Option(True, "--lint/--no-lint", help="Run lint before commit"),
    ) -> None:
        """Commit with automatic message via agent.

        Runs the commit task which stages changes (if needed), generates a
        commit message, and commits. Use -p to push after committing.
        """
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

        if lint and not run_lint(repo_root):
            typer.echo("Lint failed, aborting commit", err=True)
            raise typer.Exit(1)

        staged = subprocess.run(
            ["git", "diff", "--cached", "--quiet"],
            cwd=repo_root,
        )
        if staged.returncode == 0:
            typer.echo("Nothing staged to commit", err=True)
            raise typer.Exit(0)

        # Get the commit step prompt
        step = gather_step(repo_root, "commit")
        if not step:
            typer.echo("Error: No commit step found", err=True)
            raise typer.Exit(1)

        # Build prompt with diff context
        components = gather_prompt_components(
            repo_root,
            step="commit",
            context_config=ContextConfig.for_commit(),
        )
        prompt = format_prompt(components)

        # Run the agent to generate message and commit
        config = load_config(repo_root)
        agent_model = config.agent_model if config else "claude:opus"
        backend, model_variant = parse_model(agent_model)

        runner = get_runner(backend)
        typer.echo("Committing...")
        result = runner.launch(
            prompt,
            auto=True,
            stream=True,
            skip_permissions=True,
            model_variant=model_variant,
            cwd=repo_root,
        )

        if result.exit_code != 0:
            typer.echo("Commit failed", err=True)
            raise typer.Exit(1)

        if push:
            if has_upstream(repo_root):
                result = subprocess.run(["git", "push"], cwd=repo_root)
                if result.returncode == 0:
                    typer.echo("Pushed to origin")
                    # Create draft PR if none exists
                    url = ensure_draft_pr(repo_root)
                    if url:
                        typer.echo(f"Created draft PR: {url}")
                else:
                    typer.echo("Push failed", err=True)
                    raise typer.Exit(1)
            else:
                typer.echo("No upstream branch, skipping push", err=True)

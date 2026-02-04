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
from loopflow.lf.ops.trace import Tracer, hash_prompt, mock_response, trace_enabled


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
        tracing = trace_enabled()
        tracer = Tracer("commit", {"add": add, "lint": lint, "push": push}) if tracing else None

        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        # Check if repo is dirty
        if tracing:
            tracer.trace("git:status", result=mock_response("git:status"))
            is_dirty = mock_response("git:status") == "dirty"
        else:
            result = subprocess.run(
                ["git", "status", "--porcelain"],
                cwd=repo_root,
                capture_output=True,
                text=True,
            )
            is_dirty = bool(result.stdout.strip())

        if not is_dirty:
            if tracing:
                typer.echo(tracer.to_json())
            else:
                typer.echo("Nothing to commit", err=True)
            raise typer.Exit(0)

        # Stage changes
        if add:
            if tracing:
                tracer.trace("git:add_all")
            else:
                subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)

        # Run lint
        if lint:
            if tracing:
                lint_result = mock_response("lint:run")
                tracer.trace("lint:run", result=lint_result)
                lint_passed = lint_result == "pass"
            else:
                lint_passed = run_lint(repo_root)

            if not lint_passed:
                if tracing:
                    typer.echo(tracer.to_json())
                else:
                    typer.echo("Lint failed, aborting commit", err=True)
                raise typer.Exit(1)

        # Check if anything staged
        if tracing:
            tracer.trace("git:diff_cached", result=mock_response("git:diff_cached"))
            has_staged = mock_response("git:diff_cached") == "has_changes"
        else:
            staged = subprocess.run(
                ["git", "diff", "--cached", "--quiet"],
                cwd=repo_root,
            )
            has_staged = staged.returncode != 0

        if not has_staged:
            if tracing:
                typer.echo(tracer.to_json())
            else:
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

        if tracing:
            tracer.trace("agent:run", prompt_hash=hash_prompt(prompt), model=agent_model)
            mock_msg = mock_response("agent:commit_message")
            tracer.trace("git:commit", message=f"lf test: {mock_msg['title']}")
        else:
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
            if tracing:
                has_up = mock_response("git:has_upstream")
                tracer.trace("git:has_upstream", result=str(has_up).lower())
            else:
                has_up = has_upstream(repo_root)

            if has_up:
                if tracing:
                    tracer.trace("git:push")
                    tracer.trace("gh:pr_exists", result=str(mock_response("gh:pr_exists")).lower())
                    if not mock_response("gh:pr_exists"):
                        tracer.trace("gh:pr_create_draft")
                else:
                    result = subprocess.run(["git", "push"], cwd=repo_root)
                    if result.returncode == 0:
                        typer.echo("Pushed to origin")
                        url = ensure_draft_pr(repo_root)
                        if url:
                            typer.echo(f"Created draft PR: {url}")
                    else:
                        typer.echo("Push failed", err=True)
                        raise typer.Exit(1)
            elif not tracing:
                typer.echo("No upstream branch, skipping push", err=True)

        if tracing:
            typer.echo(tracer.to_json())

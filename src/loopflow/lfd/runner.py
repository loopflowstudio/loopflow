"""Agent pipeline runner.

Executes an agent's pipeline with prompt injection and merge handling.
"""

import os
import secrets
import subprocess
import sys
import uuid
from datetime import datetime
from pathlib import Path

from loopflow.config import load_config, parse_model
from loopflow.context import PromptComponents, gather_prompt_components, format_prompt
from loopflow.git import find_main_repo, get_current_branch
from loopflow.launcher import build_model_command, get_runner
from loopflow.lfd.client import log_session_start, log_session_end
from loopflow.lfd.db import _get_db, update_run_status
from loopflow.lfd.models import AgentSpec, AgentStatus, MergeMode, Session, SessionStatus
from loopflow.llm_http import generate_pr_message
from loopflow.logging import write_prompt_file
from loopflow.worktrees import WorktreeError, create as create_worktree


def _random_suffix() -> str:
    """Generate a short random suffix for temp branch names."""
    return secrets.token_hex(3)  # e.g. "x7k2m3"


def run_agent_iteration(
    agent: AgentSpec,
    run_id: str,
    iteration: int,
    repo_root: Path,
    foreground: bool = False,
) -> int:
    """Run one iteration of an agent's pipeline.

    Creates a worktree from personal-main, runs the pipeline tasks, handles merging.
    Returns exit code (0 for success).
    """
    main_repo = find_main_repo(repo_root) or repo_root
    config = load_config(main_repo)

    # Generate temp branch name from personal-main
    temp_branch = f"{agent.name}-{_random_suffix()}"
    base_branch = agent.personal_main

    # If no personal_main assigned, fall back to old behavior
    if not base_branch:
        temp_branch = _generate_branch_name(agent, iteration)
        base_branch = None

    try:
        worktree_path = create_worktree(main_repo, temp_branch, base=base_branch)
    except WorktreeError as e:
        print(f"Error creating worktree: {e}")
        return 1

    # Update run with worktree info
    conn = _get_db()
    conn.execute(
        "UPDATE agent_runs SET worktree = ? WHERE id = ?",
        (str(worktree_path), run_id),
    )
    conn.commit()
    conn.close()

    # Get model configuration
    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)

    runner = get_runner(backend)
    if not runner.is_available():
        print(f"Error: '{backend}' CLI not found")
        update_run_status(run_id, AgentStatus.ERROR)
        return 1

    skip_permissions = config.yolo if config else False
    exclude = list(config.exclude) if config and config.exclude else None

    # Combine config context with agent context
    all_context = list(config.context) if config and config.context else []
    all_context.extend(agent.context)

    # Parse pipeline tasks
    tasks = _parse_pipeline(agent.pipeline, config)

    # Run each task in the pipeline
    total = len(tasks)
    for i, task_name in enumerate(tasks):
        if foreground:
            print(f"\n{'='*60}")
            print(f"[{i+1}/{total}] {task_name}")
            print(f"{'='*60}\n")

        # Gather prompt components for this task
        components = gather_prompt_components(
            worktree_path,
            task=task_name,
            context=all_context or None,
            exclude=exclude,
            include_tests_for=config.include_tests_for if config else None,
            run_mode="auto",
            include_loopflow_doc=config.include_loopflow_doc if config else True,
        )

        # Inject agent prompt
        components = _inject_agent_prompt(components, agent)
        prompt = format_prompt(components)
        prompt_file = write_prompt_file(prompt)

        # Create session for tracking
        session = Session(
            id=str(uuid.uuid4()),
            task=f"{agent.name}:{task_name}",
            repo=str(main_repo),
            worktree=str(worktree_path),
            status=SessionStatus.RUNNING,
            started_at=datetime.now(),
            pid=None,
            model=backend,
            run_mode="auto",
        )
        log_session_start(session)

        # Build and run command
        chrome_enabled = config.chrome if config else False
        command = build_model_command(
            backend,
            auto=True,
            stream=True,
            skip_permissions=skip_permissions,
            model_variant=model_variant,
            sandbox_root=worktree_path.parent,
            workdir=worktree_path,
            images=components.image_files,
            chrome=chrome_enabled,
        )

        collector_cmd = [
            sys.executable,
            "-m",
            "loopflow.lfd.collector",
            "--session-id",
            session.id,
            "--task",
            f"{agent.name}:{task_name}",
            "--repo-root",
            str(worktree_path),
            "--prompt-file",
            prompt_file,
            "--autocommit",
        ]
        if foreground:
            collector_cmd.append("--foreground")
        collector_cmd.extend(["--", *command])

        process = subprocess.Popen(collector_cmd, cwd=worktree_path)
        result_code = process.wait()

        # Clean up prompt file
        try:
            os.unlink(prompt_file)
        except OSError:
            pass

        status = SessionStatus.COMPLETED if result_code == 0 else SessionStatus.ERROR
        log_session_end(session.id, status)

        if result_code != 0:
            print(f"\n[{task_name}] failed with exit code {result_code}")
            update_run_status(run_id, AgentStatus.ERROR)
            return result_code

    # Handle merge strategy
    exit_code = _handle_merge(agent, worktree_path)

    return exit_code


def _generate_branch_name(agent: AgentSpec, iteration: int) -> str:
    """Generate a branch name for an agent iteration."""
    if agent.emoji:
        return f"{agent.emoji}/{agent.name}/{iteration}"
    return f"agent/{agent.name}/{iteration}"


def _parse_pipeline(pipeline: str, config) -> list[str]:
    """Parse pipeline into list of task names.

    Pipeline can be a named pipeline from config or comma-separated tasks.
    """
    if config and config.pipelines and pipeline in config.pipelines:
        return config.pipelines[pipeline].tasks
    return [t.strip() for t in pipeline.split(",")]


def _inject_agent_prompt(
    components: PromptComponents,
    agent: AgentSpec,
) -> PromptComponents:
    """Inject agent prompt into the prompt components."""
    if not agent.prompt:
        return components

    parts = [agent.prompt]

    original_task = components.task
    if original_task:
        task_name, task_content = original_task
        parts.append("---")
        parts.append(task_content)
        combined_content = "\n\n".join(parts)
        modified_task = (task_name, combined_content)
    else:
        combined_content = "\n\n".join(parts)
        modified_task = (agent.name, combined_content)

    return PromptComponents(
        run_mode=components.run_mode,
        docs=components.docs,
        diff=components.diff,
        diff_files=components.diff_files,
        task=modified_task,
        repo_root=components.repo_root,
        clipboard=components.clipboard,
        loopflow_doc=components.loopflow_doc,
        voices=components.voices,
        image_files=components.image_files,
    )


def _handle_merge(agent: AgentSpec, worktree_path: Path) -> int:
    """Handle merge mode after pipeline completion.

    With personal-main workflow:
    - AUTO: Create PR to personal-main, auto-merge
    - PR: Create PR to personal-main, wait for approval
    - SILENT: Merge directly to personal-main without PR
    """
    branch = get_current_branch(worktree_path)
    if not branch:
        return 1

    # If no personal_main, fall back to legacy behavior
    if not agent.personal_main:
        if agent.merge_mode == MergeMode.AUTO:
            return _handle_auto_merge_legacy(worktree_path)
        elif agent.merge_mode == MergeMode.PR:
            return _handle_pr_merge_legacy(agent, worktree_path, branch)
        return 0

    # Personal-main workflow
    if agent.merge_mode == MergeMode.SILENT:
        return _handle_silent_merge(agent, worktree_path, branch)
    elif agent.merge_mode == MergeMode.AUTO:
        return _handle_auto_merge(agent, worktree_path, branch)
    elif agent.merge_mode == MergeMode.PR:
        return _handle_pr_merge(agent, worktree_path, branch)
    return 0


def _handle_silent_merge(agent: AgentSpec, worktree_path: Path, branch: str) -> int:
    """Merge directly to personal-main without creating a PR.

    Uses git fetch/push to merge without disrupting main repo's checkout state.
    """
    # Push the iteration branch first
    result = subprocess.run(
        ["git", "push", "-u", "origin", branch],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"Failed to push branch: {result.stderr}")
        return 1

    # Fetch to ensure we have latest personal-main
    subprocess.run(
        ["git", "fetch", "origin", agent.personal_main],
        cwd=worktree_path,
        capture_output=True,
    )

    # Merge iteration branch into personal-main locally in the worktree
    # First, create a local tracking branch for personal-main if needed
    result = subprocess.run(
        ["git", "branch", "--track", agent.personal_main, f"origin/{agent.personal_main}"],
        cwd=worktree_path,
        capture_output=True,
    )
    # Ignore errors - branch may already exist

    # Do the merge using git merge-base and git commit-tree for a fast-forward or merge
    # Simpler approach: checkout personal-main in the worktree, merge, push
    result = subprocess.run(
        ["git", "checkout", agent.personal_main],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"Failed to checkout {agent.personal_main}: {result.stderr}")
        return 1

    # Merge the iteration branch
    result = subprocess.run(
        ["git", "merge", "--no-ff", branch, "-m", f"Merge {branch}"],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"Failed to merge {branch}: {result.stderr}")
        return 1

    # Push personal-main
    result = subprocess.run(
        ["git", "push", "origin", agent.personal_main],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"Failed to push {agent.personal_main}: {result.stderr}")
        # Continue anyway, push can be done later

    # Delete iteration branch (local and remote)
    subprocess.run(["git", "branch", "-D", branch], cwd=worktree_path, capture_output=True)
    subprocess.run(["git", "push", "origin", "--delete", branch], cwd=worktree_path, capture_output=True)

    print(f"Merged {branch} to {agent.personal_main}")
    return 0


def _handle_auto_merge(agent: AgentSpec, worktree_path: Path, branch: str) -> int:
    """Create PR to personal-main and auto-merge it."""
    # Push the iteration branch
    result = subprocess.run(
        ["git", "push", "-u", "origin", branch],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"Failed to push branch: {result.stderr}")
        return 1

    # Create PR targeting personal-main
    try:
        message = generate_pr_message(worktree_path)
        title = f"[{agent.name}] {message.title}"
        body = f"Agent: {agent.name}\n\n{message.body}"

        cmd = [
            "gh", "pr", "create",
            "--title", title,
            "--body", body,
            "--base", agent.personal_main,
        ]
        result = subprocess.run(cmd, cwd=worktree_path, capture_output=True, text=True)

        if result.returncode == 0:
            pr_url = result.stdout.strip()
            print(f"PR created: {pr_url}")

            # Auto-merge the PR
            merge_result = subprocess.run(
                ["gh", "pr", "merge", "--squash", "--delete-branch"],
                cwd=worktree_path,
                capture_output=True,
                text=True,
            )
            if merge_result.returncode == 0:
                print(f"PR merged to {agent.personal_main}")
            else:
                print(f"PR created but auto-merge failed: {merge_result.stderr}")
                # Not a failure - PR exists for manual review
        elif "already exists" in result.stderr:
            print("PR already exists")
        else:
            print(f"Failed to create PR: {result.stderr}")
            return 1
    except Exception as e:
        print(f"Failed to create PR: {e}")
        return 1

    return 0


def _handle_pr_merge(agent: AgentSpec, worktree_path: Path, branch: str) -> int:
    """Create PR to personal-main and wait for approval."""
    # Push the iteration branch
    result = subprocess.run(
        ["git", "push", "-u", "origin", branch],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"Failed to push branch: {result.stderr}")
        return 1

    # Create PR targeting personal-main
    try:
        message = generate_pr_message(worktree_path)
        title = f"[{agent.name}] {message.title}"
        body = f"Agent: {agent.name}\n\n{message.body}"

        cmd = [
            "gh", "pr", "create",
            "--title", title,
            "--body", body,
            "--base", agent.personal_main,
        ]
        result = subprocess.run(cmd, cwd=worktree_path, capture_output=True, text=True)

        if result.returncode == 0:
            print(f"PR created: {result.stdout.strip()}")
        elif "already exists" in result.stderr:
            print("PR already exists")
        else:
            print(f"Failed to create PR: {result.stderr}")
            return 1
    except Exception as e:
        print(f"Failed to create PR: {e}")
        return 1

    return 0


# Legacy handlers for agents without personal-main

def _handle_auto_merge_legacy(worktree_path: Path) -> int:
    """Land commits directly to main (legacy behavior)."""
    result = subprocess.run(
        ["wt", "merge", "--no-squash"],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print(f"Failed to land commits: {result.stderr}")
        return 1

    print("Commits landed to main")
    return 0


def _handle_pr_merge_legacy(agent: AgentSpec, worktree_path: Path, branch: str) -> int:
    """Create a PR for this iteration (legacy behavior)."""
    # Push the branch
    result = subprocess.run(
        ["git", "push", "-u", "origin", branch],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"Failed to push branch: {result.stderr}")
        return 1

    # Create PR
    try:
        message = generate_pr_message(worktree_path)
        title = f"[{agent.name}] {message.title}"
        body = f"Agent: {agent.name}\n\n{message.body}"

        cmd = [
            "gh", "pr", "create",
            "--title", title,
            "--body", body,
        ]
        result = subprocess.run(cmd, cwd=worktree_path, capture_output=True, text=True)

        if result.returncode == 0:
            print(f"PR created: {result.stdout.strip()}")
        elif "already exists" in result.stderr:
            print("PR already exists")
        else:
            print(f"Failed to create PR: {result.stderr}")
            return 1
    except Exception as e:
        print(f"Failed to create PR: {e}")
        return 1

    return 0

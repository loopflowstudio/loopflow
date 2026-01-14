"""Agent loop runner.

Executes a registered agent's pipeline with prompt injection and outer loop handling.
Supports both single-iteration and continuous modes.
"""

import os
import signal
import subprocess
import sys
import time
import uuid
from datetime import date, datetime
from pathlib import Path

from loopflow.config import load_config, parse_model
from loopflow.context import PromptComponents, gather_prompt_components, format_prompt
from loopflow.git import find_main_repo, get_current_branch
from loopflow.launcher import build_model_command, get_runner
from loopflow.llm_http import generate_pr_message
from loopflow.logging import write_prompt_file
from loopflow.maestro import Session, SessionStatus
from loopflow.maestro.agent import (
    AgentLoopSpec,
    AgentStatus,
    OuterLoopMode,
    RegisteredAgent,
    TriggerKind,
)
from loopflow.maestro.db import (
    DEFAULT_DB_PATH,
    increment_agent_iteration,
    load_agent,
    save_session,
    update_agent_status,
    update_session_status,
)
from loopflow.worktrees import WorktreeError, create as create_worktree


# Shutdown flag for continuous mode
_shutdown_requested = False


def _get_main_head(repo_root: Path) -> str | None:
    """Get the current HEAD sha of main branch."""
    result = subprocess.run(
        ["git", "rev-parse", "origin/main"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    return result.stdout.strip()


def _should_run(agent: RegisteredAgent, repo_root: Path, last_main_sha: str | None) -> tuple[bool, str | None]:
    """Check if agent should run based on its trigger.

    Returns (should_run, new_main_sha).
    """
    trigger = agent.spec.trigger

    if trigger.kind == TriggerKind.ALWAYS:
        return True, last_main_sha

    if trigger.kind == TriggerKind.MAIN_CHANGED:
        # Fetch latest from origin
        subprocess.run(
            ["git", "fetch", "origin", "main"],
            cwd=repo_root,
            capture_output=True,
        )
        current_sha = _get_main_head(repo_root)
        if current_sha and current_sha != last_main_sha:
            return True, current_sha
        return False, last_main_sha

    return False, last_main_sha


def run_agent_continuous(
    agent: RegisteredAgent,
    repo_root: Path,
    check_interval: int = 300,
    max_iterations: int | None = None,
) -> int:
    """Run agent continuously until stopped. Returns 0 for normal shutdown."""
    global _shutdown_requested
    _shutdown_requested = False

    def _handle_shutdown(signum, frame):
        global _shutdown_requested
        _shutdown_requested = True
        print(f"\nAgent {agent.spec.name} received shutdown signal, stopping after current iteration...")

    signal.signal(signal.SIGTERM, _handle_shutdown)
    signal.signal(signal.SIGINT, _handle_shutdown)

    main_repo = find_main_repo(repo_root) or repo_root
    iterations = 0
    consecutive_failures = 0
    last_main_sha = _get_main_head(main_repo)
    today = date.today()
    today_iterations = 0

    print(f"Agent {agent.spec.name} starting in continuous mode")
    print(f"  Trigger: {agent.spec.trigger.kind.value}")
    print(f"  Check interval: {check_interval}s")
    if max_iterations:
        print(f"  Max iterations: {max_iterations}")

    while not _shutdown_requested:
        # Reset daily counter at midnight
        current_date = date.today()
        if current_date != today:
            today = current_date
            today_iterations = 0

        # Check daily rate limit
        if agent.spec.max_iterations_per_day:
            if today_iterations >= agent.spec.max_iterations_per_day:
                print(f"Daily limit reached ({agent.spec.max_iterations_per_day}), waiting until tomorrow...")
                update_agent_status(DEFAULT_DB_PATH, agent.id, AgentStatus.WAITING)
                time.sleep(3600)  # Check again in an hour
                continue

        # Check trigger condition
        should_run, new_sha = _should_run(agent, main_repo, last_main_sha)
        last_main_sha = new_sha

        if not should_run:
            update_agent_status(DEFAULT_DB_PATH, agent.id, AgentStatus.WAITING)
            time.sleep(check_interval)
            continue

        # Reload agent to get latest state
        agent = load_agent(DEFAULT_DB_PATH, agent.id)
        if not agent or agent.status == AgentStatus.STOPPED:
            break

        # Run one iteration
        print(f"\n{'='*60}")
        print(f"Starting iteration {iterations + 1}")
        print(f"{'='*60}\n")

        exit_code = run_agent_iteration(agent, main_repo, foreground=True)
        iterations += 1
        today_iterations += 1

        if exit_code != 0:
            consecutive_failures += 1
            print(f"Iteration failed (exit code {exit_code}), failure #{consecutive_failures}")

            # Back off on repeated failures
            if consecutive_failures >= 3:
                print("Too many consecutive failures, stopping agent")
                update_agent_status(DEFAULT_DB_PATH, agent.id, AgentStatus.ERROR)
                return exit_code

            backoff = min(check_interval * (2 ** consecutive_failures), 3600)
            print(f"Backing off for {backoff}s...")
            time.sleep(backoff)
            continue

        # Reset failure counter on success
        consecutive_failures = 0

        if max_iterations and iterations >= max_iterations:
            print(f"Reached max iterations ({max_iterations}), stopping")
            break

        # Respect minimum interval
        min_interval = agent.spec.min_interval_seconds
        print(f"Iteration complete, waiting {min_interval}s before next check...")
        time.sleep(min_interval)

    update_agent_status(DEFAULT_DB_PATH, agent.id, AgentStatus.IDLE)
    print(f"\nAgent {agent.spec.name} stopped after {iterations} iterations")
    return 0


def _read_agent_prompt(spec: AgentLoopSpec, repo_root: Path) -> str:
    """Read the agent's prompt file, resolving relative to repo root."""
    prompt_path = spec.prompt_path
    if not prompt_path.is_absolute():
        prompt_path = repo_root / prompt_path

    if not prompt_path.exists():
        raise FileNotFoundError(f"Agent prompt file not found: {prompt_path}")

    return prompt_path.read_text()


def _inject_agent_prompt(
    components: PromptComponents,
    agent_prompt: str,
    spec: AgentLoopSpec,
) -> PromptComponents:
    """Inject agent prompt into the prompt components.

    The agent prompt is prepended as a system directive before the task.
    """
    parts = [agent_prompt]

    original_task = components.task
    if original_task:
        task_name, task_content = original_task
        parts.append("---")
        parts.append(task_content)
        combined_content = "\n\n".join(parts)
        modified_task = (task_name, combined_content)
    else:
        # If no task, use agent prompt as the task
        combined_content = "\n\n".join(parts)
        modified_task = (spec.name, combined_content)

    return PromptComponents(
        run_mode=components.run_mode,
        docs=components.docs,
        diff=components.diff,
        task=modified_task,
        context_files=components.context_files,
        repo_root=components.repo_root,
        clipboard=components.clipboard,
    )


def _generate_branch_name(agent: RegisteredAgent, iteration: int) -> str:
    """Generate a branch name for an agent iteration."""
    return f"agent/{agent.spec.name}/{iteration}"


def run_agent_iteration(
    agent: RegisteredAgent,
    repo_root: Path,
    foreground: bool = False,
) -> int:
    """Run one iteration of an agent loop.

    Creates a worktree, runs the pipeline, and handles the outer loop.
    Returns exit code (0 for success).
    """
    main_repo = find_main_repo(repo_root) or repo_root
    config = load_config(main_repo)

    # Read agent prompt
    try:
        agent_prompt = _read_agent_prompt(agent.spec, main_repo)
    except FileNotFoundError as e:
        print(f"Error: {e}")
        return 1

    # Generate branch name and create worktree
    iteration = agent.iteration + 1
    branch_name = _generate_branch_name(agent, iteration)

    # Determine base branch for PR chaining
    base_branch = None
    if agent.spec.outer_loop.mode == OuterLoopMode.PR_CHAIN and agent.current_branch:
        base_branch = agent.current_branch

    try:
        worktree_path = create_worktree(main_repo, branch_name, base=base_branch)
    except WorktreeError as e:
        print(f"Error creating worktree: {e}")
        return 1

    # Update agent state
    update_agent_status(
        DEFAULT_DB_PATH,
        agent.id,
        AgentStatus.RUNNING,
        pid=os.getpid(),
        worktree=worktree_path,
        branch=branch_name,
    )

    # Get model configuration
    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)

    runner = get_runner(backend)
    if not runner.is_available():
        print(f"Error: '{backend}' CLI not found")
        update_agent_status(DEFAULT_DB_PATH, agent.id, AgentStatus.ERROR)
        return 1

    skip_permissions = config.yolo if config else False
    exclude = list(config.exclude) if config and config.exclude else None

    # Combine config context with agent context
    all_context = list(config.context) if config and config.context else []
    all_context.extend(agent.spec.context)

    # Run each task in the pipeline
    total = len(agent.spec.pipeline)
    for i, task_name in enumerate(agent.spec.pipeline):
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
        components = _inject_agent_prompt(components, agent_prompt, agent.spec)
        prompt = format_prompt(components)
        prompt_file = write_prompt_file(prompt)

        # Create session for tracking
        session = Session(
            id=str(uuid.uuid4()),
            task=f"{agent.spec.name}:{task_name}",
            repo=main_repo,
            worktree=worktree_path,
            status=SessionStatus.RUNNING,
            started_at=datetime.now(),
            pid=None,
            backend=backend,
            run_mode="auto",
        )
        save_session(DEFAULT_DB_PATH, session)

        # Build and run command
        command = build_model_command(
            backend,
            auto=True,
            stream=True,
            skip_permissions=skip_permissions,
            model_variant=model_variant,
            sandbox_root=worktree_path.parent,
            workdir=worktree_path,
        )

        collector_cmd = [
            sys.executable,
            "-m",
            "loopflow.maestro.collector",
            "--session-id",
            session.id,
            "--task",
            f"{agent.spec.name}:{task_name}",
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
        session.pid = process.pid
        save_session(DEFAULT_DB_PATH, session)
        result_code = process.wait()

        # Clean up prompt file
        try:
            os.unlink(prompt_file)
        except OSError:
            pass  # Best effort cleanup

        status = SessionStatus.COMPLETED if result_code == 0 else SessionStatus.ERROR
        update_session_status(DEFAULT_DB_PATH, session.id, status)

        if result_code != 0:
            print(f"\n[{task_name}] failed with exit code {result_code}")
            update_agent_status(DEFAULT_DB_PATH, agent.id, AgentStatus.ERROR)
            return result_code

    # Handle outer loop
    exit_code = _handle_outer_loop(agent, worktree_path)

    # Update agent state
    increment_agent_iteration(DEFAULT_DB_PATH, agent.id)
    update_agent_status(DEFAULT_DB_PATH, agent.id, AgentStatus.IDLE)

    return exit_code


def _handle_outer_loop(agent: RegisteredAgent, worktree_path: Path) -> int:
    """Handle the outer loop strategy after pipeline completion."""
    mode = agent.spec.outer_loop.mode

    if mode == OuterLoopMode.PR_CHAIN:
        return _handle_pr_chain(agent, worktree_path)
    elif mode == OuterLoopMode.LAND_COMMITS:
        return _handle_land_commits(worktree_path)

    return 0


def _handle_pr_chain(agent: RegisteredAgent, worktree_path: Path) -> int:
    """Create a PR for this iteration, chaining from previous if exists.

    Following Graphite-like UX: each iteration's PR depends on the previous.
    """
    branch = get_current_branch(worktree_path)
    if not branch:
        return 1

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

    # Determine base branch for the PR
    # If this is not the first iteration and the previous branch exists, use it as base
    base_branch = "main"
    if agent.current_branch and agent.iteration > 0:
        # Check if previous branch exists on origin
        check_result = subprocess.run(
            ["git", "ls-remote", "--heads", "origin", agent.current_branch],
            cwd=worktree_path,
            capture_output=True,
            text=True,
        )
        if check_result.returncode == 0 and check_result.stdout.strip():
            base_branch = agent.current_branch

    # Create PR
    try:
        message = generate_pr_message(worktree_path)
        title = f"[{agent.spec.name}] {message.title}"
        body = f"Agent: {agent.spec.name}\nIteration: {agent.iteration + 1}\n\n{message.body}"

        # Use gh CLI to create PR with specific base
        cmd = [
            "gh", "pr", "create",
            "--title", title,
            "--body", body,
            "--base", base_branch,
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


def _handle_land_commits(worktree_path: Path) -> int:
    """Land commits directly to main using wt merge."""
    # Use worktrunk merge for landing
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


def stop_agent(agent_id: str) -> bool:
    """Stop a running or waiting agent by its ID."""
    agent = load_agent(DEFAULT_DB_PATH, agent_id)
    if not agent:
        return False

    if agent.status not in (AgentStatus.RUNNING, AgentStatus.WAITING):
        return False

    if agent.pid:
        try:
            os.kill(agent.pid, signal.SIGTERM)
        except OSError:
            pass

    update_agent_status(DEFAULT_DB_PATH, agent_id, AgentStatus.STOPPED)
    return True

"""Core step execution logic.

This module provides the unified execution path for running steps.
Both single-step (`lf <step>`) and flow execution (`lf flow`) use this.
"""

import os
import subprocess
import sys
import uuid
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from loopflow.lf.context import (
    PromptComponents,
    format_context_prompt,
    format_prompt,
    format_task_prompt,
)
from loopflow.lf.git import find_main_repo
from loopflow.lf.launcher import build_model_command, build_model_interactive_command
from loopflow.lf.logging import get_model_env, write_prompt_file, write_prompt_log
from loopflow.lf.output import print_step_header, warn_if_context_too_large
from loopflow.lf.tokens import analyze_components
from loopflow.lfd.agent import log_agent_end, log_agent_start
from loopflow.lfd.models import Agent, AgentStatus


@dataclass
class ExecutionParams:
    """Parameters for step execution."""

    step_name: str
    repo_root: Path
    components: PromptComponents
    backend: str
    model_variant: str | None
    skip_permissions: bool
    chrome: bool = False

    # Mode
    is_interactive: bool = False
    use_execvp: bool = False  # True = replace process (single-step), False = subprocess

    # Output formatting
    step_num: int | None = None
    total_steps: int | None = None
    direction: list[str] | None = None
    context: list[str] | None = None

    # Flow lineage for prompt log filenames
    flow_parents: list[str] | None = None

    # Post-execution
    autocommit: bool = True
    push: bool = False


def execute_step(params: ExecutionParams) -> int:
    """Execute a step and return exit code.

    This is the core execution function used by both single-step and flow execution.

    For interactive mode with use_execvp=True (single-step), this function does not return
    as it replaces the current process.

    For interactive mode with use_execvp=False (flow), uses subprocess.run to preserve
    the flow's ability to continue after the step.

    For auto mode, uses the collector subprocess for output capture and logging.
    """
    tree = analyze_components(params.components)
    warn_if_context_too_large(tree)

    # Split prompt into context (system) and task (user message)
    full_prompt = format_prompt(params.components)
    context_prompt = format_context_prompt(params.components)
    task_prompt = format_task_prompt(params.components)
    token_summary = tree.format()

    main_repo = find_main_repo(params.repo_root) or params.repo_root
    run_mode = "interactive" if params.is_interactive else "auto"

    agent = Agent(
        id=str(uuid.uuid4()),
        step=params.step_name,
        repo=str(main_repo),
        worktree=str(params.repo_root),
        status=AgentStatus.RUNNING,
        started_at=datetime.now(),
        pid=os.getpid() if params.is_interactive and params.use_execvp else None,
        model=params.backend,
        run_mode=run_mode,
    )
    log_agent_start(agent)

    # Print header
    if params.is_interactive:
        step_display = f"{params.step_name} (interactive)"
    else:
        step_display = params.step_name
    print_step_header(
        step_display,
        params.backend,
        params.model_variant,
        direction=params.direction,
        context=params.context,
        step_num=params.step_num,
        total_steps=params.total_steps,
        token_summary=token_summary,
    )

    # Write full prompt to .lf/log/ for debugging
    write_prompt_log(
        params.repo_root,
        full_prompt,
        params.step_name,
        params.flow_parents,
    )

    # Write context separately for backends that support split loading
    context_file = write_prompt_log(
        params.repo_root,
        context_prompt,
        f"{params.step_name}.context",
        params.flow_parents,
    )

    if params.is_interactive:
        return _execute_interactive(params, agent, task_prompt, context_file)
    else:
        return _execute_auto(params, agent, task_prompt, context_file)


def _execute_interactive(
    params: ExecutionParams,
    agent: Agent,
    task_prompt: str,
    context_file: Path,
) -> int:
    """Execute step in interactive mode.

    Context is loaded via context_file using each backend's native mechanism:
    - Claude: --append-system-prompt-file
    - Codex: -c model_instructions_file="..."
    - Gemini: GEMINI_SYSTEM_MD env var
    Task is passed as the CLI argument (user message).
    """
    command = build_model_interactive_command(
        params.backend,
        skip_permissions=params.skip_permissions,
        yolo=params.skip_permissions,
        model_variant=params.model_variant,
        sandbox_root=params.repo_root.parent,
        workdir=params.repo_root,
        images=params.components.image_files,
        chrome=params.chrome,
        context_file=context_file,
    )

    cmd_with_prompt = command + [task_prompt]

    gemini_context = context_file if params.backend == "gemini" else None
    env = get_model_env(gemini_context_file=gemini_context)

    if params.use_execvp:
        # Single-step: replace current process (doesn't return)
        os.chdir(params.repo_root)
        os.environ.clear()
        os.environ.update(env)
        os.execvp(cmd_with_prompt[0], cmd_with_prompt)
        # This line is never reached
        return 0

    # Flow: use subprocess.run to allow flow to continue
    result = subprocess.run(cmd_with_prompt, cwd=params.repo_root, env=env)

    status = AgentStatus.COMPLETED if result.returncode == 0 else AgentStatus.FAILED
    log_agent_end(agent.id, status)

    if result.returncode != 0:
        print(f"\n[{params.step_name}] failed with exit code {result.returncode}")

    return result.returncode


def _execute_auto(
    params: ExecutionParams,
    agent: Agent,
    task_prompt: str,
    context_file: Path,
) -> int:
    """Execute step in auto mode using collector.

    Context is loaded via context_file using each backend's native mechanism:
    - Claude: --append-system-prompt-file
    - Codex: -c model_instructions_file="..."
    - Gemini: GEMINI_SYSTEM_MD env var
    Task is passed as the CLI argument (user message).
    """
    command = build_model_command(
        params.backend,
        auto=True,
        stream=True,
        skip_permissions=params.skip_permissions,
        yolo=params.skip_permissions,
        model_variant=params.model_variant,
        sandbox_root=params.repo_root.parent,
        workdir=params.repo_root,
        images=params.components.image_files,
        chrome=params.chrome,
        context_file=context_file,
    )

    # Write prompt to temp file for collector
    prompt_file = write_prompt_file(task_prompt)

    collector_cmd = [
        sys.executable,
        "-m",
        "loopflow.lfd.execution.collector",
        "--agent-id",
        agent.id,
        "--step",
        params.step_name,
        "--repo-root",
        str(params.repo_root),
        "--prompt-file",
        prompt_file,
        "--foreground",
    ]
    if params.autocommit:
        collector_cmd.append("--autocommit")
    if params.push:
        collector_cmd.append("--push")
    collector_cmd.extend(["--", *command])

    gemini_context = context_file if params.backend == "gemini" else None
    env = get_model_env(gemini_context_file=gemini_context)

    process = subprocess.Popen(collector_cmd, cwd=params.repo_root, env=env)
    result_code = process.wait()

    # Clean up temp file (prompt log is preserved in .lf/log/)
    try:
        os.unlink(prompt_file)
    except OSError:
        pass

    status = AgentStatus.COMPLETED if result_code == 0 else AgentStatus.FAILED
    log_agent_end(agent.id, status)

    if result_code != 0:
        print(f"\n[{params.step_name}] failed with exit code {result_code}")

    return result_code

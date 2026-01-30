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

from loopflow.lf.context import PromptComponents, format_prompt
from loopflow.lf.git import find_main_repo
from loopflow.lf.launcher import build_model_command, build_model_interactive_command
from loopflow.lf.logging import write_prompt_file
from loopflow.lf.output import print_step_header, warn_if_context_too_large
from loopflow.lf.tokens import analyze_components
from loopflow.lfd.models import StepRun, StepRunStatus
from loopflow.lfd.step_run import log_step_run_end, log_step_run_start


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

    prompt = format_prompt(params.components)
    token_summary = tree.format()

    main_repo = find_main_repo(params.repo_root) or params.repo_root
    run_mode = "interactive" if params.is_interactive else "auto"

    step_run = StepRun(
        id=str(uuid.uuid4()),
        step=params.step_name,
        repo=str(main_repo),
        worktree=str(params.repo_root),
        status=StepRunStatus.RUNNING,
        started_at=datetime.now(),
        pid=os.getpid() if params.is_interactive and params.use_execvp else None,
        model=params.backend,
        run_mode=run_mode,
    )
    log_step_run_start(step_run)

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

    if params.is_interactive:
        return _execute_interactive(params, step_run, prompt)
    else:
        return _execute_auto(params, step_run, prompt)


def _execute_interactive(
    params: ExecutionParams,
    step_run: StepRun,
    prompt: str,
) -> int:
    """Execute step in interactive mode."""
    command = build_model_interactive_command(
        params.backend,
        skip_permissions=params.skip_permissions,
        yolo=params.skip_permissions,
        model_variant=params.model_variant,
        sandbox_root=params.repo_root.parent,
        workdir=params.repo_root,
        images=params.components.image_files,
        chrome=params.chrome,
    )
    cmd_with_prompt = command + [prompt]

    # Remove API keys so CLIs use subscriptions
    env = os.environ.copy()
    env.pop("ANTHROPIC_API_KEY", None)
    env.pop("OPENAI_API_KEY", None)

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

    status = StepRunStatus.COMPLETED if result.returncode == 0 else StepRunStatus.FAILED
    log_step_run_end(step_run.id, status)

    if result.returncode != 0:
        print(f"\n[{params.step_name}] failed with exit code {result.returncode}")

    return result.returncode


def _execute_auto(
    params: ExecutionParams,
    step_run: StepRun,
    prompt: str,
) -> int:
    """Execute step in auto mode using collector."""
    prompt_file = write_prompt_file(prompt)

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
    )

    collector_cmd = [
        sys.executable,
        "-m",
        "loopflow.lfd.execution.collector",
        "--step-run-id",
        step_run.id,
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

    process = subprocess.Popen(collector_cmd, cwd=params.repo_root)
    result_code = process.wait()

    # Clean up prompt file
    try:
        os.unlink(prompt_file)
    except OSError:
        pass

    status = StepRunStatus.COMPLETED if result_code == 0 else StepRunStatus.FAILED
    log_step_run_end(step_run.id, status)

    if result_code != 0:
        print(f"\n[{params.step_name}] failed with exit code {result_code}")

    return result_code

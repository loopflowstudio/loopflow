"""Pipeline execution for chaining tasks."""

import os
import platform
import subprocess
import sys
import uuid
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Optional

from loopflow.config import parse_model
from loopflow.context import build_prompt
from loopflow.git import GitError, find_main_repo, open_pr
from loopflow.launcher import build_model_command, get_runner
from loopflow.lfd.pipelines import PipelineDef, resolve_pipeline, StepConfig
from loopflow.llm_http import generate_pr_message
from loopflow.logging import write_prompt_file
from loopflow.lfd.client import log_session_start, log_session_end
from loopflow.lfd.models import Session, SessionStatus


@dataclass
class _StepParams:
    """Parameters for executing a single pipeline step."""
    task: str
    backend: str
    model_variant: str | None
    context: list[str] | None
    voices: list[str] | None


def _run_step(
    params: _StepParams,
    repo_root: Path,
    main_repo: Path,
    exclude: list[str] | None,
    skip_permissions: bool,
    should_push: bool,
    step_num: int,
    total_steps: int,
) -> int:
    """Execute a single pipeline step. Returns exit code."""
    print(f"\n{'='*60}")
    print(f"[{step_num}/{total_steps}] {params.task}")
    print(f"{'='*60}\n")

    prompt = build_prompt(
        repo_root,
        params.task,
        context=params.context,
        exclude=exclude,
        run_mode="auto",
        voices=params.voices,
    )
    prompt_file = write_prompt_file(prompt)

    session = Session(
        id=str(uuid.uuid4()),
        task=params.task,
        repo=str(main_repo),
        worktree=str(repo_root),
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
        pid=None,
        model=params.backend,
        run_mode="auto",
    )
    log_session_start(session)

    command = build_model_command(
        params.backend,
        auto=True,
        stream=True,
        skip_permissions=skip_permissions,
        model_variant=params.model_variant,
        sandbox_root=repo_root.parent,
        workdir=repo_root,
    )
    collector_cmd = [
        sys.executable,
        "-m",
        "loopflow.lfd.collector",
        "--session-id",
        session.id,
        "--task",
        params.task,
        "--repo-root",
        str(repo_root),
        "--prompt-file",
        prompt_file,
        "--autocommit",
        "--foreground",
    ]
    if should_push:
        collector_cmd.append("--push")
    collector_cmd.extend(["--", *command])

    process = subprocess.Popen(collector_cmd, cwd=repo_root)
    result_code = process.wait()

    os.unlink(prompt_file)

    status = SessionStatus.COMPLETED if result_code == 0 else SessionStatus.ERROR
    log_session_end(session.id, status)

    if result_code != 0:
        print(f"\n[{params.task}] failed with exit code {result_code}")

    return result_code


def _finalize_pipeline(
    pipeline_name: str,
    repo_root: Path,
    should_pr: bool,
) -> None:
    """Handle post-pipeline tasks: PR creation and notification."""
    if should_pr:
        try:
            message = generate_pr_message(repo_root)
            pr_url = open_pr(repo_root, title=message.title, body=message.body)
            print(f"\nPR created: {pr_url}")
        except GitError as e:
            print(f"\nPR creation failed: {e}")

    _notify_done(pipeline_name)


def _notify_done(pipeline_name: str) -> None:
    """Show macOS notification. No-op on other platforms."""
    if platform.system() != "Darwin":
        return
    try:
        subprocess.run(
            ["osascript", "-e", f'display notification "Pipeline complete" with title "lf {pipeline_name}"'],
            capture_output=True,
        )
    except FileNotFoundError:
        pass


def run_pipeline_def(
    pipeline: PipelineDef,
    repo_root: Path,
    context: Optional[list[str]] = None,
    exclude: Optional[list[str]] = None,
    skip_permissions: bool = False,
    push_enabled: bool = False,
    pr_enabled: bool = False,
    backend: str = "claude",
    model_variant: str | None = "opus",
) -> int:
    """Run a PipelineDef (from .lf/pipelines/). Returns first non-zero exit code, or 0."""
    should_push = push_enabled
    should_pr = pr_enabled
    if should_pr:
        should_push = True

    runner = get_runner(backend)
    if not runner.is_available():
        print(f"Error: '{backend}' CLI not found")
        return 1

    main_repo = find_main_repo(repo_root) or repo_root
    resolved = resolve_pipeline(pipeline, repo_root)
    total = len(resolved)

    for i, step in enumerate(resolved):
        # Per-step config can override model/voice/context
        step_backend = backend
        step_variant = model_variant
        step_context = list(context) if context else []
        step_voices = None

        if step.config:
            if step.config.model:
                step_backend, step_variant = parse_model(step.config.model)
            if step.config.context:
                step_context.extend(step.config.context)
            if step.config.voice:
                step_voices = [step.config.voice]

        params = _StepParams(
            task=step.task,
            backend=step_backend,
            model_variant=step_variant,
            context=step_context or None,
            voices=step_voices,
        )
        result_code = _run_step(
            params, repo_root, main_repo, exclude,
            skip_permissions, should_push, i + 1, total,
        )
        if result_code != 0:
            return result_code

    _finalize_pipeline(pipeline.name, repo_root, should_pr)
    return 0

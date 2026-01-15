"""Pipeline execution for chaining tasks."""

import os
import platform
import subprocess
import sys
import uuid
from datetime import datetime
from pathlib import Path
from typing import Optional

from loopflow.config import PipelineConfig
from loopflow.context import build_prompt
from loopflow.git import GitError, find_main_repo, open_pr
from loopflow.launcher import build_model_command, get_runner
from loopflow.llm_http import generate_pr_message
from loopflow.logging import write_prompt_file
from loopflow.lfd.client import log_session_start, log_session_end
from loopflow.lfd.models import Session, SessionStatus


def run_pipeline(
    pipeline: PipelineConfig,
    repo_root: Path,
    context: Optional[list[str]] = None,
    exclude: Optional[list[str]] = None,
    include_tests_for: Optional[list[str]] = None,
    skip_permissions: bool = False,
    push_enabled: bool = False,
    pr_enabled: bool = False,
    backend: str = "claude",
    model_variant: str | None = "opus",
) -> int:
    """Run each task in sequence. Returns first non-zero exit code, or 0."""
    # Pipeline settings override globals
    should_push = pipeline.push if pipeline.push is not None else push_enabled
    should_pr = pipeline.pr if pipeline.pr is not None else pr_enabled

    # PR implies push
    if should_pr:
        should_push = True

    runner = get_runner(backend)
    if not runner.is_available():
        print(f"Error: '{backend}' CLI not found")
        return 1

    main_repo = find_main_repo(repo_root) or repo_root

    total = len(pipeline.tasks)
    for i, task_name in enumerate(pipeline.tasks):
        # Task header
        print(f"\n{'='*60}")
        print(f"[{i+1}/{total}] {task_name}")
        print(f"{'='*60}\n")

        prompt = build_prompt(
            repo_root,
            task_name,
            context=context,
            exclude=exclude,
            include_tests_for=include_tests_for,
            run_mode="auto",
        )
        prompt_file = write_prompt_file(prompt)

        session = Session(
            id=str(uuid.uuid4()),
            task=task_name,
            repo=str(main_repo),
            worktree=str(repo_root),
            status=SessionStatus.RUNNING,
            started_at=datetime.now(),
            pid=None,
            model=backend,
            run_mode="auto",
        )
        log_session_start(session)

        command = build_model_command(
            backend,
            auto=True,
            stream=True,
            skip_permissions=skip_permissions,
            model_variant=model_variant,
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
            task_name,
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
        # Don't strip API keys from collector env - it needs them for commit message generation
        process = subprocess.Popen(collector_cmd, cwd=repo_root)
        result_code = process.wait()

        # Clean up prompt file
        os.unlink(prompt_file)

        status = SessionStatus.COMPLETED if result_code == 0 else SessionStatus.ERROR
        log_session_end(session.id, status)

        if result_code != 0:
            print(f"\n[{task_name}] failed with exit code {result_code}")
            return result_code

    if should_pr:
        try:
            message = generate_pr_message(repo_root)
            pr_url = open_pr(repo_root, title=message.title, body=message.body)
            print(f"\nPR created: {pr_url}")
        except GitError as e:
            print(f"\nPR creation failed: {e}")

    _notify_done(pipeline.name)
    return 0


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
        pass  # osascript not available

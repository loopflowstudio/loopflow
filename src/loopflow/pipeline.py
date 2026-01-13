"""Pipeline execution for chaining tasks."""

import os
import subprocess
import sys
import tempfile
import uuid
from datetime import datetime
from pathlib import Path
from typing import Optional

from loopflow.config import PipelineConfig
from loopflow.context import build_prompt
from loopflow.git import GitError, find_main_repo, open_pr
from loopflow.launcher import build_model_command, get_runner
from loopflow.llm_http import generate_pr_message
from loopflow.logging import get_model_env
from loopflow.maestro import Session, SessionStatus
from loopflow.maestro.db import DEFAULT_DB_PATH, save_session, update_session_status


def _write_prompt_file(prompt: str) -> str:
    """Write prompt to a temp file and return the path."""
    fd, path = tempfile.mkstemp(prefix="lf-prompt-", suffix=".txt")
    os.write(fd, prompt.encode())
    os.close(fd)
    return path


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
        prompt_file = _write_prompt_file(prompt)

        session = Session(
            id=str(uuid.uuid4()),
            task=task_name,
            repo=main_repo,
            worktree=repo_root,
            status=SessionStatus.RUNNING,
            started_at=datetime.now(),
            pid=None,
            backend=backend,
            run_mode="auto",
        )
        save_session(DEFAULT_DB_PATH, session)

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
            "loopflow.maestro.collector",
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
        process = subprocess.Popen(collector_cmd, cwd=repo_root, env=get_model_env())
        session.pid = process.pid
        save_session(DEFAULT_DB_PATH, session)
        result_code = process.wait()

        # Clean up prompt file
        os.unlink(prompt_file)

        status = SessionStatus.COMPLETED if result_code == 0 else SessionStatus.ERROR
        update_session_status(DEFAULT_DB_PATH, session.id, status)

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
    """Show macOS notification."""
    subprocess.run([
        "osascript", "-e",
        f'display notification "Pipeline complete" with title "lf {pipeline_name}"'
    ])

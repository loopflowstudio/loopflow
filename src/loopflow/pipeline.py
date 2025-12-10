"""Pipeline execution for chaining tasks."""

import subprocess
from dataclasses import dataclass
from pathlib import Path

from loopflow.context import build_prompt
from loopflow.launcher import launch_claude


@dataclass
class Pipeline:
    name: str
    tasks: list[str]


def run_pipeline(
    pipeline: Pipeline,
    repo_root: Path,
    arg: str | None = None,
    context: list[str] | None = None,
    skip_permissions: bool = False,
) -> int:
    """Run each task in sequence. Returns first non-zero exit code, or 0."""
    for i, task_name in enumerate(pipeline.tasks):
        # Only first task gets the arg
        task_arg = arg if i == 0 else None

        prompt = build_prompt(repo_root, task_name, arg=task_arg, context=context)
        exit_code, _ = launch_claude(
            prompt,
            print_mode=True,
            stream=True,
            skip_permissions=skip_permissions,
            cwd=repo_root,
        )

        if exit_code != 0:
            return exit_code

        _autocommit(repo_root, task_name, task_arg)

    _notify_done(pipeline.name)
    return 0


def _autocommit(repo_root: Path, task: str, arg: str | None) -> None:
    """Commit changes with the lf command as the message."""
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if not result.stdout.strip():
        return  # Nothing to commit

    msg = f"lf {task}"
    if arg:
        msg += f" {arg}"

    subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", msg], cwd=repo_root, check=True)


def _notify_done(pipeline_name: str) -> None:
    """Show macOS notification."""
    subprocess.run([
        "osascript", "-e",
        f'display notification "Pipeline complete" with title "lf {pipeline_name}"'
    ])

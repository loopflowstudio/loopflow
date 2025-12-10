"""Pipeline execution for chaining tasks."""

import subprocess
from dataclasses import dataclass
from pathlib import Path

from loopflow.context import build_prompt
from loopflow.git import has_upstream, open_pr
from loopflow.git import push as git_push
from loopflow.launcher import launch_claude


@dataclass
class Pipeline:
    name: str
    tasks: list[str]
    push: bool | None = None
    pr: bool | None = None


def run_pipeline(
    pipeline: Pipeline,
    repo_root: Path,
    arg: str | None = None,
    context: list[str] | None = None,
    skip_permissions: bool = False,
    push_enabled: bool = False,
    pr_enabled: bool = False,
) -> int:
    """Run each task in sequence. Returns first non-zero exit code, or 0."""
    # Pipeline settings override globals
    should_push = pipeline.push if pipeline.push is not None else push_enabled
    should_pr = pipeline.pr if pipeline.pr is not None else pr_enabled

    # PR implies push
    if should_pr:
        should_push = True

    total = len(pipeline.tasks)
    for i, task_name in enumerate(pipeline.tasks):
        # Only first task gets the arg
        task_arg = arg if i == 0 else None

        # Task header
        print(f"\n{'='*60}")
        print(f"[{i+1}/{total}] {task_name}")
        print(f"{'='*60}\n")

        prompt = build_prompt(repo_root, task_name, arg=task_arg, context=context)
        exit_code, _ = launch_claude(
            prompt,
            print_mode=True,
            stream=True,
            skip_permissions=skip_permissions,
            cwd=repo_root,
        )

        if exit_code != 0:
            print(f"\n[{task_name}] failed with exit code {exit_code}")
            return exit_code

        _autocommit(repo_root, task_name, task_arg, push=should_push)

    if should_pr:
        pr_url = open_pr(repo_root, draft=True)
        if pr_url:
            print(f"\nPR created: {pr_url}")

    _notify_done(pipeline.name)
    return 0


def _autocommit(repo_root: Path, task: str, arg: str | None, push: bool = False) -> None:
    """Commit changes, optionally push."""
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if not result.stdout.strip():
        print(f"\n[{task}] no changes to commit")
        return

    msg = f"lf {task}"
    if arg:
        msg += f" {arg}"

    subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", msg], cwd=repo_root, check=True)
    print(f"\n[{task}] committed: {msg}")

    if push and has_upstream(repo_root):
        git_push(repo_root)
        print(f"[{task}] pushed to origin")


def _notify_done(pipeline_name: str) -> None:
    """Show macOS notification."""
    subprocess.run([
        "osascript", "-e",
        f'display notification "Pipeline complete" with title "lf {pipeline_name}"'
    ])

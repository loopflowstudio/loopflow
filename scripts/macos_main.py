"""Settle a post-merge macOS run on main.

Green (or no macOS job reached): force `last-green` to the tested commit.
Red: bisect main's first-parent commits since `last-green` by dispatching
macos-main.yml on candidates, then open an unarmed revert PR for the culprit
and note the failure on it and on the culprit's own PR.

    macos_main.py RESULT LAST_GREEN SHA
"""

import json
import os
import subprocess
import sys
import time
from collections.abc import Callable
from pathlib import Path

_JOBS = ("swift-test", "macos-process-test", "loopflow-ui-test")
_WORKFLOW = "macos-main.yml"
_REVERT_PREFIX = "macos-revert/"
_POLL_SECONDS = 60


def _git(*args: str) -> str:
    return subprocess.run(["git", *args], capture_output=True, text=True, check=True).stdout.strip()


def _gh(*args: str) -> str:
    return subprocess.run(["gh", *args], capture_output=True, text=True, check=True).stdout.strip()


def _summary(text: str) -> None:
    print(text)
    with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as output:
        output.write(text + "\n")


def find_culprit(commits: list[str], passes: Callable[[str], bool]) -> str:
    """Return the first failing commit; the last is known red, the base before them green."""
    good, bad = -1, len(commits) - 1
    while bad - good > 1:
        mid = (good + bad) // 2
        if passes(commits[mid]):
            good = mid
        else:
            bad = mid
    return commits[bad]


def failed_jobs(job_names: list[str]) -> list[str]:
    """Map failed check names such as `macos / macos-process-test (macos-15)` to job keys."""
    return [job for job in _JOBS if any(name.startswith(f"macos / {job}") for name in job_names)]


def _run_failures(repository: str, run_id: str) -> list[str]:
    failed = _gh(
        "api",
        "--paginate",
        f"repos/{repository}/actions/runs/{run_id}/jobs",
        "--jq",
        '.jobs[] | select(.conclusion == "failure") | .name',
    )
    return failed_jobs(failed.splitlines()) or list(_JOBS)


def _dispatch_passes(run_id: str, jobs: list[str]) -> Callable[[str], bool]:
    def passes(sha: str) -> bool:
        request = f"{run_id}-{sha[:12]}"
        _gh(
            "workflow",
            "run",
            _WORKFLOW,
            "--ref",
            "main",
            "-f",
            f"sha={sha}",
            "-f",
            f"jobs={' '.join(jobs)}",
            "-f",
            f"request={request}",
        )
        title = f"macOS bisect {request}"
        while True:
            time.sleep(_POLL_SECONDS)
            runs = json.loads(
                _gh(
                    "run",
                    "list",
                    "--workflow",
                    _WORKFLOW,
                    "--event",
                    "workflow_dispatch",
                    "--limit",
                    "50",
                    "--json",
                    "displayTitle,status,conclusion,url",
                )
            )
            run = next((run for run in runs if run["displayTitle"] == title), None)
            if run and run["status"] == "completed":
                print(f"{sha[:12]}: {run['conclusion']} ({run['url']})")
                return run["conclusion"] == "success"

    return passes


def _open_revert(repository: str, culprit: str, jobs: list[str], run_url: str) -> None:
    subject = _git("log", "-1", "--format=%s", culprit)
    branch = f"{_REVERT_PREFIX}{culprit[:12]}"
    _git("config", "user.name", "github-actions[bot]")
    _git("config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com")
    _git("switch", "-c", branch, "origin/main")
    try:
        _git("revert", "--no-edit", culprit)
    except subprocess.CalledProcessError:
        _git("revert", "--abort")
        _summary(f"{culprit[:12]} does not revert cleanly on main; fix forward.")
        sys.exit(1)
    _git("push", "origin", branch)
    note = (
        f"macOS {', '.join(jobs)} failed on main ({run_url}) and bisected to {culprit}.\n\n"
        "This revert is not armed. Merge it to turn main green, or close it and "
        "fix forward on a new PR (the `ci-fix` skill repairs failing checks). "
        "Actions opened it, so its CI starts once someone pushes to it or reopens it."
    )
    url = _gh(
        "pr",
        "create",
        "--base",
        "main",
        "--head",
        branch,
        "--title",
        f'Revert "{subject}"',
        "--body",
        note,
    )
    _summary(f"Opened {url}")
    pulls = json.loads(_gh("api", f"repos/{repository}/commits/{culprit}/pulls"))
    for pull in pulls:
        _gh("pr", "comment", str(pull["number"]), "--body", f"{note}\n\nRevert: {url}")


def _known_revert(commits: list[str]) -> str | None:
    """Return an open revert PR whose culprit is in this batch."""
    pulls = json.loads(_gh("pr", "list", "--state", "open", "--json", "headRefName,url"))
    for pull in pulls:
        if pull["headRefName"].startswith(_REVERT_PREFIX):
            prefix = pull["headRefName"].removeprefix(_REVERT_PREFIX)
            if any(sha.startswith(prefix) for sha in commits):
                return pull["url"]
    return None


def main() -> None:
    result, last_green, sha = sys.argv[1:4]
    if result in ("success", "skipped"):
        _git("push", "--force", "origin", f"{sha}:refs/tags/last-green")
        _summary(f"last-green → {sha}")
        return

    if result != "failure":
        _summary(f"macOS {result}; last-green stays at {last_green or 'nothing'}.")
        sys.exit(1)
    repository = os.environ["GITHUB_REPOSITORY"]
    run_id = os.environ["GITHUB_RUN_ID"]
    run_url = f"https://github.com/{repository}/actions/runs/{run_id}"
    if not last_green:
        _summary("macOS failed and no last-green commit exists to bisect from.")
        sys.exit(1)
    commits = _git("rev-list", "--first-parent", "--reverse", f"{last_green}..{sha}").split()
    if not commits:
        _summary(f"macOS failed on {sha}, which last-green already covers; rerun it.")
        sys.exit(1)
    known = _known_revert(commits)
    if known:
        _summary(f"macOS is red on main; {known} already reverts a commit in this batch.")
        sys.exit(1)
    jobs = _run_failures(repository, run_id)
    culprit = find_culprit(commits, _dispatch_passes(run_id, jobs))
    _summary(f"Culprit: {culprit} ({', '.join(jobs)})")
    _open_revert(repository, culprit, jobs, run_url)
    sys.exit(1)


if __name__ == "__main__":
    main()

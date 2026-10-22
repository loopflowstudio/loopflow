"""Cancel a pull request's superseded merge-queue CI runs.

A pull request that leaves the merge queue keeps its queue run, and GitHub
rebuilds the entries behind it on new temporary branches while their old runs
keep holding runners. A merge_group run cancels its pull request's earlier
runs; the pull request's dequeued event cancels its remaining queue runs.

    cancel_merge_queue_runs.py 1607                                  # a pull request
    cancel_merge_queue_runs.py gh-readonly-queue/main/pr-1607-<sha> --keep 42  # a queue ref
"""

import argparse
import json
import os
import re
import subprocess
import sys

_ACTIVE = ("queued", "in_progress")


def pull_request_number(entry: str) -> int:
    match = re.fullmatch(r"(?:gh-readonly-queue/[^/]+/pr-)?(\d+)(?:-[0-9a-f]+)?", entry)
    if match is None:
        raise ValueError(f"not a pull request number or merge-queue ref: {entry}")
    return int(match.group(1))


def superseded(runs: list[dict], number: int, keep: int | None) -> list[int]:
    """Active queue runs for this pull request other than `keep`."""
    prefix = re.compile(rf"gh-readonly-queue/[^/]+/pr-{number}-")
    return [
        run["id"]
        for run in runs
        if prefix.match(run["head_branch"] or "") and run["status"] in _ACTIVE and run["id"] != keep
    ]


def _api(path: str, method: str = "GET") -> str:
    return subprocess.run(
        ["gh", "api", "-X", method, path], capture_output=True, text=True, check=True
    ).stdout


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("entry", help="pull request number or merge-queue ref")
    parser.add_argument("--keep", type=int, help="run id to leave running")
    args = parser.parse_args()
    number = pull_request_number(args.entry)
    repository = os.environ["GITHUB_REPOSITORY"]
    runs: list[dict] = []
    for status in _ACTIVE:
        page = _api(
            f"repos/{repository}/actions/workflows/ci.yml/runs"
            f"?event=merge_group&status={status}&per_page=100"
        )
        runs.extend(json.loads(page)["workflow_runs"])
    cancelled = superseded(runs, number, args.keep)
    for run in cancelled:
        _api(f"repos/{repository}/actions/runs/{run}/cancel", method="POST")
        print(f"Cancelled superseded merge-queue run {run} for #{number}.")
    if not cancelled:
        print(f"No superseded merge-queue runs for #{number}.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, subprocess.CalledProcessError, ValueError, KeyError) as error:
        # Cleanup never blocks the proof it runs beside.
        print(f"Merge-queue cleanup skipped ({error}).", file=sys.stderr)

"""Find the merge queue's successful CI proof for the exact pushed commit."""

import json
import os
import subprocess
import sys
from pathlib import Path
from urllib.parse import urlencode


def find_merge_proof(repository: str, sha: str) -> str | None:
    query = urlencode(
        {"event": "merge_group", "head_sha": sha, "status": "success", "per_page": 100}
    )
    try:
        result = subprocess.run(
            ["gh", "api", f"repos/{repository}/actions/workflows/ci.yml/runs?{query}"],
            capture_output=True,
            text=True,
            check=True,
            timeout=20,
        )
        runs = json.loads(result.stdout)["workflow_runs"]
        for run in runs:
            if (
                run["head_sha"] == sha
                and run["event"] == "merge_group"
                and run["path"] == ".github/workflows/ci.yml"
                and run["status"] == "completed"
                and run["conclusion"] == "success"
                and type(run["id"]) is int
                and run["id"] > 0
            ):
                return f"https://github.com/{repository}/actions/runs/{run['id']}"
    except (OSError, subprocess.SubprocessError, ValueError, KeyError, TypeError) as error:
        print(
            f"Merge proof unavailable ({type(error).__name__}); running the full matrix.",
            file=sys.stderr,
        )
    return None


def main() -> None:
    proof = find_merge_proof(os.environ["GITHUB_REPOSITORY"], os.environ["GITHUB_SHA"])
    result = f"reuse={'true' if proof else 'false'}\nproof_url={proof or ''}\n"
    with Path(os.environ["GITHUB_OUTPUT"]).open("a") as output:
        output.write(result)
    summary = (
        f"Reuse the successful merge-group CI proof: {proof}\n"
        "Main still warms missing dependency caches.\n"
        if proof
        else "No exact successful merge-group CI proof; run the full matrix.\n"
    )
    print(summary)
    with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as output:
        output.write(summary)


if __name__ == "__main__":
    main()

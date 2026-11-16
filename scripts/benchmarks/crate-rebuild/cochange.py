"""Report first-party Rust co-change from a bounded first-parent PR sample."""

import argparse
import collections
import json
import subprocess


def _git(*args: str) -> str:
    return subprocess.check_output(["git", *args], text=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", default="HEAD")
    parser.add_argument("--count", type=int, default=80)
    args = parser.parse_args()
    rows = []
    for line in _git(
        "log", "--first-parent", f"-{args.count}", "--format=%H %s", args.revision
    ).splitlines():
        sha, title = line.split(" ", 1)
        if "(#" not in title:
            continue
        files = []
        for row in _git(
            "diff", "--numstat", "--no-renames", "-w", sha + "^", sha, "--", "rust"
        ).splitlines():
            added, deleted, path = row.split("\t")
            if not path.endswith(".rs") or added == "-":
                continue
            files.append({"path": path, "lines": int(added) + int(deleted)})
        if not files:
            continue
        # Keep broad changes visible; they are not automatically formatting.
        rows.append(
            {
                "sha": sha,
                "title": title,
                "files": files,
                "broad": len(files) > 30 or sum(f["lines"] for f in files) > 3000,
            }
        )
    application_modules = collections.Counter()
    for row in rows:
        application_modules.update(
            {
                f["path"].split("/")[3].removesuffix(".rs")
                for f in row["files"]
                if f["path"].startswith("rust/loopflow/src/")
            }
        )
    print(
        json.dumps(
            {
                "revision": _git("rev-parse", args.revision).strip(),
                "first_parent_commits": args.count,
                "rust_prs": len(rows),
                "application_module_touches": application_modules,
                "prs": rows,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()

"""Compare serial CLI builds in disposable source copies with a private cache."""

import argparse
import json
import os
import re
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def _edit_task_record(tree: Path) -> Path:
    sources = [
        path
        for path in (tree / "rust").glob("*/src/**/task/mod.rs")
        if "pub enum TaskDataError" in path.read_text()
    ]
    if len(sources) != 1:
        raise RuntimeError("cannot locate the Task record edit anchor")
    source = sources[0]
    text = source.read_text()
    old = '#[error("invalid Task: {0}")]'
    if text.count(old) != 1:
        raise RuntimeError("Task record message anchor changed")
    source.write_text(text.replace(old, '#[error("invalid Task: {0} (rebuild probe)")]'))
    return source


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--candidate", default="HEAD")
    parser.add_argument("--sccache", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=1)
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    cache = args.sccache.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("LF_", "SCCACHE_", "CARGO_"))
        and key
        not in (
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "RUSTFLAGS",
            "LOOPFLOW_BUILD_PROVENANCE",
            "LOOPFLOW_RELEASE_TAG",
            "LOOPFLOW_BUILD_SOURCE_ROOT",
            "LOOPFLOW_MIGRATION_ORDER",
        )
    }
    env.update(
        CARGO_INCREMENTAL="0",
        SCCACHE_CLIENT_SIDE="1",
        RUSTC_WRAPPER=str(cache),
        SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS=(
            "CARGO_MANIFEST_DIR,CARGO_MANIFEST_PATH,LOOPFLOW_BUILD_SOURCE_ROOT"
        ),
    )
    receipts = []

    def run(argv: list[str], cwd: Path = ROOT) -> str:
        result = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True)
        if result.returncode:
            raise RuntimeError(f"{argv!r} failed:\n{result.stdout}\n{result.stderr}")
        return result.stdout

    def stats() -> dict:
        return json.loads(run([str(cache), "--show-stats", "--stats-format", "json"]))["stats"]

    with tempfile.TemporaryDirectory(prefix="lf650-build-", dir="/tmp") as temporary:
        root = Path(temporary).resolve()
        snapshots = {}
        for label, revision in (("base", args.base), ("candidate", args.candidate)):
            sha = run(["git", "rev-parse", "--verify", revision + "^{commit}"]).strip()
            trees = [root / f"{label}-{suffix}" for suffix in ("a", "b")]
            for tree in trees:
                run(["git", "clone", "--shared", "--no-checkout", "--quiet", str(ROOT), str(tree)])
                run(["git", "checkout", "--quiet", "--detach", sha], tree)
                # The build uses the source repository's real merged migration frontier.
                run(
                    [
                        "git",
                        "fetch",
                        "--quiet",
                        str(ROOT),
                        "refs/remotes/origin/main:refs/remotes/origin/main",
                    ],
                    tree,
                )
            snapshots[label] = (sha, trees)
        env["SCCACHE_BASEDIRS"] = os.pathsep.join(
            str(tree) for _, trees in snapshots.values() for tree in trees
        )
        env["SCCACHE_SERVER_UDS"] = str(root / "cache.sock")
        env["SCCACHE_CONF"] = str(root / "sccache.toml")
        (root / "sccache.toml").write_text("")
        for label, (sha, trees) in snapshots.items():
            # Separate cold caches prevent the candidate borrowing from the baseline.
            env["SCCACHE_DIR"] = str(root / f"{label}-cache")
            try:
                run(["nice", "-n", "10", str(cache), "--start-server"])
                for case, tree in (
                    ("cold_a", trees[0]),
                    ("warm_b", trees[1]),
                    ("shipping_b", trees[1]),
                    ("task_record_b", trees[1]),
                ):
                    edited_source = None
                    if case == "task_record_b":
                        edited_source = _edit_task_record(tree)
                    if case == "shipping_b":
                        # A real string-value change in shipping logic, not a touched comment.
                        sources = [
                            p
                            for p in (tree / "rust/loopflow/src/ops/task").rglob("*.rs")
                            if "async fn task_by_issue(" in p.read_text()
                            and "pub enum ShipStart" in p.read_text()
                        ]
                        if len(sources) != 1:
                            raise RuntimeError("cannot locate the shipping edit anchor")
                        source = sources[0]
                        edited_source = source
                        old = '"no Task exists for {issue:?}"'
                        text = source.read_text()
                        if text.count(old) != 1:
                            raise RuntimeError("shipping message anchor changed")
                        source.write_text(
                            text.replace(old, '"no Task exists for {issue:?} (rebuild probe)"')
                        )
                    before = stats()
                    start = time.monotonic()
                    stem = f"{label}_{case}"
                    with (
                        (output / f"{stem}.jsonl").open("w") as stdout,
                        (output / f"{stem}.log").open("w") as stderr,
                    ):
                        result = subprocess.run(
                            [
                                "/usr/bin/time",
                                "-l" if os.uname().sysname == "Darwin" else "-v",
                                "nice",
                                "-n",
                                "10",
                                "cargo",
                                "build",
                                "--locked",
                                "-p",
                                "loopflow",
                                "--bin",
                                "lf",
                                "--jobs",
                                str(args.jobs),
                                "--message-format=json",
                            ],
                            cwd=tree,
                            env=env,
                            stdout=stdout,
                            stderr=stderr,
                        )
                    after = stats()
                    artifacts = [
                        json.loads(line)
                        for line in (output / f"{stem}.jsonl").read_text().splitlines()
                        if line.startswith("{")
                    ]
                    log = (output / f"{stem}.log").read_text()
                    receipt = {
                        "revision": sha,
                        "case": stem,
                        "exit_code": result.returncode,
                        "wall_seconds": round(time.monotonic() - start, 3),
                        "jobs": args.jobs,
                        "edited_source": (
                            str(edited_source.relative_to(tree)) if edited_source else None
                        ),
                        "nonfresh_artifacts": sorted(
                            {
                                item["target"]["name"]
                                for item in artifacts
                                if item.get("reason") == "compiler-artifact" and not item["fresh"]
                            }
                        ),
                        "stats_before": before,
                        "stats_after": after,
                        "timing": [
                            line.strip()
                            for line in log.splitlines()
                            if re.search(
                                r"real.*user.*sys|User time|System time|[Mm]aximum resident",
                                line,
                            )
                        ],
                    }
                    receipts.append(receipt)
                    (output / "receipts.json").write_text(json.dumps(receipts, indent=2) + "\n")
                    print(
                        json.dumps(
                            {
                                key: receipt[key]
                                for key in (
                                    "case",
                                    "wall_seconds",
                                    "exit_code",
                                    "nonfresh_artifacts",
                                )
                            }
                        ),
                        flush=True,
                    )
                    if result.returncode:
                        raise RuntimeError(f"build failed: see {output / (stem + '.log')}")
                    run([str(tree / "target/debug/lf"), "--version"], tree)
            finally:
                run([str(cache), "--stop-server"])


if __name__ == "__main__":
    main()

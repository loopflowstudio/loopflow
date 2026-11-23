"""Compare a seeded new worktree with a cold one through an `lf` build and a focused test."""

import json
import os
import re
import stat
import sys
import time
from pathlib import Path

from run import _timing
from test_loop import (
    FILTERS,
    ROOT,
    _require_disk,
    _run,
    _time_command,
    _write,
    add_worktree,
    measurement_run,
    remove_worktree,
)

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from build_cache import seed_record  # noqa: E402
from resource_envelope import load_policy  # noqa: E402

ARMS = ("seeded", "cold")
PACKAGE, TEST = FILTERS["app"]
COMMANDS = {
    "build": ["cargo", "build", "--locked", "-v", "-p", "loopflow", "--bin", "lf"],
    "test": ["cargo", "test", "--locked", "-v", "-p", PACKAGE, "--lib", TEST, "--", "--exact"],
}


def _samples(cohorts: int) -> list[tuple[int, str]]:
    return [(c, arm) for c in range(1, cohorts + 1) for arm in (ARMS if c % 2 else ARMS[::-1])]


def _env() -> dict:
    # The Machine's own Cargo configuration, shared cache and jobs; no inherited lf context.
    return {k: v for k, v in os.environ.items() if not k.startswith("LF_")}


def _allocations(debug: Path) -> dict[tuple[int, int], int]:
    """Allocated bytes by inode and mtime. Cargo's hard links share one allocation."""
    files = {}
    for path in debug.rglob("*"):
        info = path.lstat()
        if stat.S_ISREG(info.st_mode):
            files[info.st_ino, info.st_mtime_ns] = info.st_blocks * 512
    return files


def validate(receipts: list[dict], cohorts: int) -> None:
    if cohorts < 3 or sorted((r["cohort"], r["arm"]) for r in receipts) != sorted(
        _samples(cohorts)
    ):
        raise ValueError("three or more complete cohorts are required")
    for r in receipts:
        if r["build"]["exit_code"] or r["test"]["exit_code"] or r["test"]["tests_passed"] != 1:
            raise ValueError(f"cohort {r['cohort']} {r['arm']}: a command failed")
        if (r["seed"] is None) != (r["arm"] == "cold"):
            raise ValueError(f"cohort {r['cohort']} {r['arm']}: seed state does not match its arm")


def _sample(output: Path, root: Path, revision: str, cohort: int, arm: str, reserve: int) -> dict:
    tree = root / f"{cohort}-{arm}"
    env = _env()
    remove_worktree(tree, env)  # An interrupted attempt's checkout.
    start = time.monotonic()
    # The installed post-checkout hook seeds and registers, as for any new worktree.
    try:
        add_worktree(tree, revision, env, cold=arm == "cold")
        add_seconds = time.monotonic() - start
        seed = seed_record(tree)
        if arm == "seeded" and seed is None:
            raise RuntimeError("no quiescent sibling target to seed from; rerun with --resume")
        if seed:
            revisions = f"{seed['commit']}...{revision}"
            seed["commits_apart"] = (
                int(_run(["git", "rev-list", "--count", revisions], ROOT, env))
                if seed["commit"]
                else None
            )
        debug = tree / "target/debug"
        seeded_files = _allocations(debug)
        receipt = {
            "cohort": cohort,
            "arm": arm,
            "seed": seed,
            "add": {"wall_seconds": add_seconds},
            "seed_bytes": sum(seeded_files.values()),
        }
        before = seeded_files
        for step, command in COMMANDS.items():
            stem = f"{cohort}_{arm}_{step}"
            load_before = os.getloadavg()
            begin = time.monotonic()
            exit_code, memory = _time_command(command, tree, env, output, stem, begin, reserve)
            wall = time.monotonic() - begin
            log = (output / f"{stem}.log").read_text()
            stdout = (output / f"{stem}.jsonl").read_text()
            after = _allocations(debug)
            receipt[step] = {
                "command": command,
                "exit_code": exit_code,
                "wall_seconds": wall,
                **_timing(log),
                "tree_rss_bytes": max((rss for _, rss in memory), default=0),
                "compiled": len(re.findall(r"^\s+Compiling ", log, re.M)),
                "fresh": len(re.findall(r"^\s+Fresh ", log, re.M)),
                "build_scripts_run": len(re.findall(r"^\s+Running `\S*build-script-", log, re.M)),
                "tests_passed": sum(
                    int(n) for n in re.findall(r"test result: ok\. (\d+) passed", stdout)
                ),
                "written_bytes": sum(size for key, size in after.items() if key not in before),
                "load_before": load_before,
                "load_after": os.getloadavg(),
            }
            before = after
            print(f"{stem}: exit {exit_code}, {wall:.1f}s", flush=True)
            if exit_code:
                raise RuntimeError(f"{step} failed: {output / (stem + '.log')}")
        receipt["target_bytes"] = sum(after.values())
        receipt["still_seeded_bytes"] = sum(
            size for key, size in after.items() if key in seeded_files
        )
        return receipt
    finally:
        remove_worktree(tree, env)


def _measure(output: Path, cohorts: int, resume: bool) -> None:
    state, receipts_path = output / "configuration.json", output / "receipts.json"
    reserve = load_policy().minimum_free_disk_bytes
    _require_disk(output, reserve)
    if resume:
        config = json.loads(state.read_text())
    else:
        config = {
            "revision": _run(["git", "rev-parse", "HEAD"], ROOT, _env()),
            "cohorts": cohorts,
            "rustc": _run(["rustc", "-Vv"], ROOT, _env()),
            "cpus": os.cpu_count(),
            "machine": list(os.uname()),
            "commands": COMMANDS,
            "created": time.time(),
        }
        _write(state, config)
    receipts = json.loads(receipts_path.read_text()) if receipts_path.exists() else []
    # Same volume as the repository's worktrees, so the seed clones instead of failing.
    root = output / "worktrees"
    root.mkdir(exist_ok=True)
    for cohort, arm in _samples(config["cohorts"]):
        if any((r["cohort"], r["arm"]) == (cohort, arm) for r in receipts):
            continue
        receipts.append(_sample(output, root, config["revision"], cohort, arm, reserve))
        _write(receipts_path, receipts)
    validate(receipts, config["cohorts"])
    root.rmdir()
    print(f"Receipts: {output}")


def main() -> None:
    with measurement_run(__doc__) as (output, cohorts, resume):
        _measure(output, cohorts, resume)


if __name__ == "__main__":
    main()

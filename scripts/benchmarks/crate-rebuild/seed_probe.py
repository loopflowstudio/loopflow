"""Compare a seeded new worktree with a cold one through an `lf` build and a focused test."""

import json
import os
import re
import shutil
import stat
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

from run import _timing
from test_loop import FILTERS, _require_disk, _run, _time_command, _write, measurement_run

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from build_cache import seed_record  # noqa: E402
from resource_envelope import load_policy  # noqa: E402

ROOT = Path(__file__).resolve().parents[3]
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


@dataclass(frozen=True)
class TargetFile:
    path: Path
    inode: int
    mtime_ns: int
    allocated_bytes: int = field(compare=False)


def _files(debug: Path) -> set[TargetFile]:
    files = set()
    for path in debug.rglob("*"):
        info = path.lstat()
        if stat.S_ISREG(info.st_mode):
            files.add(TargetFile(path, info.st_ino, info.st_mtime_ns, info.st_blocks * 512))
    return files


def _allocated(files: set[TargetFile]) -> int:
    # Uplifted files are hard links to their deps/ originals; count each inode once.
    return sum({file.inode: file.allocated_bytes for file in files}.values())


def _remove_tree(tree: Path) -> None:
    if tree.exists():
        _run(["git", "worktree", "remove", "--force", str(tree)], ROOT, _env())


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
    _remove_tree(tree)  # An interrupted attempt's checkout.
    start = time.monotonic()
    # The installed post-checkout hook seeds and registers, as for any new worktree.
    env = _env()
    _run(["git", "worktree", "add", "--detach", str(tree), revision], ROOT, env)
    try:
        if arm == "cold" and (tree / "target").exists():
            shutil.rmtree(tree / "target")
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
        seeded_files = _files(debug)
        receipt = {
            "cohort": cohort,
            "arm": arm,
            "seed": seed,
            "add": {"wall_seconds": add_seconds},
            "seed_bytes": _allocated(seeded_files),
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
            after = _files(debug)
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
                "written_bytes": _allocated(after - before),
                "load_before": load_before,
                "load_after": os.getloadavg(),
            }
            before = after
            print(f"{stem}: exit {exit_code}, {wall:.1f}s", flush=True)
            if exit_code:
                raise RuntimeError(f"{step} failed: {output / (stem + '.log')}")
        receipt["target_bytes"] = _allocated(after)
        receipt["still_seeded_bytes"] = _allocated({file for file in after if file in seeded_files})
        return receipt
    finally:
        _remove_tree(tree)


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

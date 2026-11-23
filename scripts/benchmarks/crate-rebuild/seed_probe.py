"""Compare a seeded new worktree with a cold one through an `lf` build and a focused test."""

import argparse
import fcntl
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

from run import _timing
from test_loop import FILTERS, _require_disk, _time_command, _write

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from build_cache import SEED_RECORD  # noqa: E402
from resource_envelope import load_policy  # noqa: E402

ROOT = Path(__file__).resolve().parents[3]
ARMS = ("seeded", "cold")
PACKAGE, TEST = FILTERS["app"]
COMMANDS = {
    "build": ["cargo", "build", "--locked", "-v", "-p", "loopflow", "--bin", "lf"],
    "test": ["cargo", "test", "--locked", "-v", "-p", PACKAGE, "--lib", TEST, "--", "--exact"],
}
METRICS = (
    "wall_seconds",
    "cpu_seconds",
    "rss_bytes",
    "tree_rss_bytes",
    "compiled",
    "fresh",
    "written_bytes",
)


def _samples(cohorts: int) -> list[tuple[int, str]]:
    return [(c, arm) for c in range(1, cohorts + 1) for arm in (ARMS if c % 2 else ARMS[::-1])]


def _env() -> dict:
    # The Machine's own Cargo configuration, shared cache and jobs; no inherited lf context.
    return {k: v for k, v in os.environ.items() if not k.startswith("LF_")}


def _run(argv: list[str], cwd: Path) -> str:
    result = subprocess.run(
        argv, cwd=cwd, env=_env(), text=True, capture_output=True, stdin=subprocess.DEVNULL
    )
    if result.returncode:
        raise RuntimeError(f"{argv}: {result.stdout}\n{result.stderr}")
    return result.stdout.strip()


def _files(debug: Path) -> dict[str, tuple[int, int, int]]:
    """Path → (inode, mtime, allocated bytes) for every regular file under the target."""
    rows = {}
    for directory, _, names in os.walk(debug):
        for name in names:
            path = os.path.join(directory, name)
            info = os.lstat(path)
            if os.path.isfile(path) and not os.path.islink(path):
                rows[os.path.relpath(path, debug)] = (
                    info.st_ino,
                    info.st_mtime_ns,
                    info.st_blocks * 512,
                )
    return rows


def _allocated(rows: dict, keys) -> int:
    # Uplifted files are hard links to their deps/ originals; count each inode once.
    return sum({rows[k][0]: rows[k][2] for k in keys}.values())


def _written(before: dict, after: dict) -> int:
    """Allocated bytes of files a command created or replaced."""
    return _allocated(after, [k for k, v in after.items() if before.get(k, (0, 0))[:2] != v[:2]])


def _remove_tree(tree: Path) -> None:
    if tree.exists():
        _run(["git", "worktree", "remove", "--force", str(tree)], ROOT)


def summarize(receipts: list[dict]) -> list[dict]:
    rows = []
    for arm in ARMS:
        for step in ("add", *COMMANDS):
            samples = [r for r in receipts if r["arm"] == arm]
            if not samples:
                continue
            row = {"arm": arm, "step": step, "n": len(samples)}
            for metric in ("wall_seconds",) if step == "add" else METRICS:
                values = [r[step][metric] for r in samples]
                row[metric] = {
                    "median": statistics.median(values),
                    "min": min(values),
                    "max": max(values),
                }
            rows.append(row)
    return rows


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
    _run(["git", "worktree", "add", "--detach", str(tree), revision], ROOT)
    add_seconds = time.monotonic() - start
    try:
        seed = None
        if arm == "cold":
            # The documented way to build a new worktree cold; its time counts toward the add.
            shutil.rmtree(tree / "target", ignore_errors=True)
            add_seconds = time.monotonic() - start
        elif (tree / "target" / SEED_RECORD).exists():
            seed = json.loads((tree / "target" / SEED_RECORD).read_text())
            seed["commits_apart"] = (
                int(_run(["git", "rev-list", "--count", f"{seed['commit']}...{revision}"], ROOT))
                if seed["commit"]
                else None
            )
        else:
            raise RuntimeError("no quiescent sibling target to seed from; rerun with --resume")
        debug = tree / "target/debug"
        seeded_files = _files(debug)
        receipt = {
            "cohort": cohort,
            "arm": arm,
            "seed": seed,
            "add": {"wall_seconds": add_seconds},
            "seed_bytes": _allocated(seeded_files, seeded_files),
        }
        before = seeded_files
        for step, command in COMMANDS.items():
            stem = f"{cohort}_{arm}_{step}"
            load_before = os.getloadavg()
            begin = time.monotonic()
            exit_code, memory = _time_command(command, tree, _env(), output, stem, begin, reserve)
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
                "written_bytes": _written(before, after),
                "load_before": load_before,
                "load_after": os.getloadavg(),
            }
            before = after
            print(f"{stem}: exit {exit_code}, {wall:.1f}s", flush=True)
            if exit_code:
                raise RuntimeError(f"{step} failed: {output / (stem + '.log')}")
        unchanged = [k for k, v in seeded_files.items() if before.get(k, (0, 0))[:2] == v[:2]]
        receipt["target_bytes"] = _allocated(before, before)
        receipt["still_seeded_bytes"] = _allocated(before, unchanged)
        return receipt
    finally:
        _remove_tree(tree)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cohorts", type=int, default=3)
    parser.add_argument("--resume", action="store_true")
    args = parser.parse_args()
    if args.cohorts < 3:
        parser.error("at least three cohorts are required")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    state, receipts_path = output / "configuration.json", output / "receipts.json"
    if state.exists() != args.resume:
        parser.error("use --resume with an existing output directory, and only then")
    lock = os.open(output, os.O_RDONLY)
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        parser.error(f"measurement already running in {output}")
    reserve = load_policy().minimum_free_disk_bytes
    _require_disk(output, reserve)
    if args.resume:
        config = json.loads(state.read_text())
    else:
        config = {
            "revision": _run(["git", "rev-parse", "HEAD"], ROOT),
            "cohorts": args.cohorts,
            "rustc": _run(["rustc", "-Vv"], ROOT),
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
        _write(output / "summary.json", summarize(receipts))
    validate(receipts, config["cohorts"])
    root.rmdir()
    print(f"Receipts: {output}")


if __name__ == "__main__":
    main()

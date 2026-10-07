"""Profile `lf wt list` end to end against a real repository.

Each sample runs one listing with Git's trace2 event stream enabled and a
timing shim in front of `gh`, then splits the cost into local Git, remote
enrichment and everything else. Nothing here mutates the repository: the
listing is the default read-only one.

    uv run python scripts/benchmarks/wt-list/profile.py \
        --lf target/release/lf --repo ~/src/loopflow --samples 10 --home fresh

`--home fresh` points LF_HOME at a new empty directory so a branch build never
touches the main Machine. `--home main` measures the installed CLI where it runs.
`--store ~/.lf/loopflow.db` copies that database into the fresh Machine first, so
the listing pays for a store of real size without writing to the original.
"""

import argparse
import json
import os
import shutil
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

REMOTE_GIT = {"ls-remote", "fetch", "remote-https", "remote-http", "remote-ssh"}

GH_SHIM = """#!/bin/sh
start=$(python3 -c 'import time; print(time.time())')
env -u GIT_TRACE2_EVENT "{gh}" "$@"
code=$?
end=$(python3 -c 'import time; print(time.time())')
echo "$start $end" >> "{log}"
exit $code
"""


def _percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, round(fraction * (len(ordered) - 1)))]


def _git_processes(trace: Path) -> list[tuple[str, float]]:
    """(subcommand, seconds) for every git process in a trace2 event file."""
    names: dict[str, str] = {}
    elapsed: dict[str, float] = {}
    if not trace.exists():
        return []
    for line in trace.read_text().splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        sid = event.get("sid", "")
        if event.get("event") == "cmd_name":
            names.setdefault(sid, event.get("name", "?"))
        elif event.get("event") == "atexit":
            elapsed[sid] = float(event.get("t_abs", 0.0))
    # A child sid is "<parent>/<child>"; count each process once.
    return [(names.get(sid, "?"), seconds) for sid, seconds in elapsed.items()]


def _gh_calls(log: Path) -> list[float]:
    if not log.exists():
        return []
    return [
        float(end) - float(start)
        for start, end in (line.split() for line in log.read_text().splitlines())
    ]


def _run(command: list[str], cwd: Path, env: dict[str, str]) -> float:
    started = time.perf_counter()
    subprocess.run(command, cwd=cwd, env=env, stdout=subprocess.DEVNULL, check=True)
    return time.perf_counter() - started


def _copy_store(store: Path, home: Path) -> None:
    """Copy a database and its write-ahead log; a clone where the filesystem allows."""
    home.mkdir(parents=True)
    for suffix in ("", "-wal"):
        source = Path(f"{store}{suffix}")
        if not source.exists():
            continue
        target = home / f"loopflow.db{suffix}"
        if subprocess.run(["cp", "-c", source, target], capture_output=True).returncode != 0:
            shutil.copyfile(source, target)


def profile(
    lf: Path, repo: Path, samples: int, mode: str, home: str, offline: bool, store: Path | None
) -> dict:
    work = Path(tempfile.mkdtemp(prefix="wt-list-profile-"))
    shim_dir = work / "bin"
    shim_dir.mkdir()
    gh_log = work / "gh.log"
    real_gh = shutil.which("gh")
    env = dict(os.environ)
    for name in [name for name in env if name.startswith("LF_")]:
        del env[name]
    if real_gh:
        shim = shim_dir / "gh"
        shim.write_text(GH_SHIM.format(gh=real_gh, log=gh_log))
        shim.chmod(0o755)
        env["PATH"] = f"{shim_dir}:{env['PATH']}"
    if home == "fresh":
        env["LF_HOME"] = str(work / "home")
        if store:
            _copy_store(store, work / "home")
    if offline:
        # Unroutable proxy: every remote call fails instead of reaching GitHub.
        for name in ("https_proxy", "HTTPS_PROXY", "http_proxy", "HTTP_PROXY", "ALL_PROXY"):
            env[name] = "http://127.0.0.1:9"
        env["GIT_SSH_COMMAND"] = "false"

    command = [str(lf), "wt", "list"] + (["--json"] if mode == "json" else [])
    _run(command, repo, env)  # warm caches; not a sample

    rows = []
    for _ in range(samples):
        trace = work / "trace2.jsonl"
        trace.unlink(missing_ok=True)
        gh_log.unlink(missing_ok=True)
        wall = _run(command, repo, {**env, "GIT_TRACE2_EVENT": str(trace)})
        git = _git_processes(trace)
        gh = _gh_calls(gh_log)
        rows.append(
            {
                "wall": wall,
                "git_local_count": sum(1 for name, _ in git if name not in REMOTE_GIT),
                "git_local_seconds": sum(s for name, s in git if name not in REMOTE_GIT),
                "git_remote_count": sum(1 for name, _ in git if name in REMOTE_GIT),
                "git_remote_seconds": sum(s for name, s in git if name in REMOTE_GIT),
                "gh_count": len(gh),
                "gh_seconds": sum(gh),
            }
        )
    # The same executable and Machine with no repository work: process start,
    # Exec admission and both ledger writes.
    floor = [_run([str(lf), "machine", "id"], repo, env) for _ in range(samples)]
    shutil.rmtree(work, ignore_errors=True)

    walls = [row["wall"] for row in rows]

    def median(key: str) -> float:
        return statistics.median(row[key] for row in rows)

    return {
        "lf": str(lf),
        "repo": str(repo),
        "mode": mode,
        "home": home,
        "store_mb": round(store.stat().st_size / 1e6) if store and home == "fresh" else None,
        "offline": offline,
        "samples": samples,
        "load_average_1m": round(os.getloadavg()[0], 1),
        "worktrees": len(
            [
                line
                for line in subprocess.run(
                    ["git", "-C", str(repo), "worktree", "list", "--porcelain"],
                    capture_output=True,
                    text=True,
                    check=True,
                ).stdout.splitlines()
                if line.startswith("worktree ")
            ]
        ),
        "wall_median_s": round(statistics.median(walls), 3),
        "wall_p95_s": round(_percentile(walls, 0.95), 3),
        "wall_max_s": round(max(walls), 3),
        "git_local_processes": median("git_local_count"),
        "git_local_summed_s": round(median("git_local_seconds"), 3),
        "git_remote_processes": median("git_remote_count"),
        "git_remote_summed_s": round(median("git_remote_seconds"), 3),
        "gh_processes": median("gh_count"),
        "gh_summed_s": round(median("gh_seconds"), 3),
        "journal_floor_median_s": round(statistics.median(floor), 3),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--lf", type=Path, required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=10)
    parser.add_argument("--mode", choices=["json", "text", "both"], default="both")
    parser.add_argument("--home", choices=["fresh", "main"], default="fresh")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--store", type=Path, help="database to copy into the fresh Machine")
    args = parser.parse_args()
    modes = ["text", "json"] if args.mode == "both" else [args.mode]
    for mode in modes:
        result = profile(
            args.lf.expanduser().resolve(),
            args.repo.expanduser().resolve(),
            args.samples,
            mode,
            args.home,
            args.offline,
            args.store.expanduser().resolve() if args.store else None,
        )
        print(json.dumps(result))


if __name__ == "__main__":
    main()

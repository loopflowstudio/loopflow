"""Measure serial edits through passing focused tests in linked worktrees."""

import argparse
import fcntl
import hashlib
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from collections.abc import Sequence
from pathlib import Path

import tomlkit
from run import _timing

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from build_cache import cargo_configuration, configured_cache, daemon_pid, refresh  # noqa: E402
from resource_envelope import load_policy  # noqa: E402

ROOT = Path(__file__).resolve().parents[3]
FILTERS = {
    "app": ("loopflow", "lf::tests::task_deletion_accepts_issue_identity"),
    "execution": (
        "loopflow-execution",
        "skills::catalog::tests::portable_fallback_is_unique_or_reports_candidates",
    ),
}
CASES = [
    "app_new",
    "app_edit_2",
    "app_edit_3",
    "execution_after_app",
    "execution_edit_2",
    "execution_edit_3",
]


def _write(path: Path, value: object) -> None:
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def _run(argv: list[str], cwd: Path, env: dict) -> str:
    result = subprocess.run(
        argv, cwd=cwd, env=env, text=True, capture_output=True, stdin=subprocess.DEVNULL
    )
    if result.returncode:
        raise RuntimeError(f"{argv}: {result.stdout}\n{result.stderr}")
    return result.stdout.strip()


def _edit(tree: Path, owner: str, suffix: str) -> str:
    if owner == "app":
        path = tree / "rust/loopflow/src/lf/mod.rs"
        old = 'about = "Open Loopflow or run its CLI"'
    else:
        paths = [
            p
            for p in (tree / "rust").glob("*/src/ops/task/**/*.rs")
            if "pub enum ShipStart" in p.read_text()
        ]
        if len(paths) != 1:
            raise RuntimeError("cannot locate execution edit")
        path = paths[0]
        old = '"no Task exists for {issue:?}"'
    # Always start from the committed source: repeated edits replace the previous edit.
    relative = str(path.relative_to(tree))
    original = subprocess.check_output(["git", "show", f"HEAD:{relative}"], cwd=tree, text=True)
    if original.count(old) != 1:
        raise RuntimeError(f"cannot locate semantic edit in {path}")
    position = old.rfind('"')
    path.write_text(original.replace(old, old[:position] + f" ({suffix})" + old[position:]))
    return relative


def _processes() -> dict[int, tuple[int, int, str]]:
    raw = subprocess.check_output(["ps", "-axo", "pid=,ppid=,rss=,time="], text=True)
    return {
        int(p): (int(parent), int(rss) * 1024, cpu)
        for line in raw.splitlines()
        if len(parts := line.split()) == 4
        for p, parent, rss, cpu in [parts]
    }


def _cpu(value: str) -> float:
    parts = value.replace("-", ":").split(":")
    return sum(float(part) * scale for part, scale in zip(reversed(parts), (1, 60, 3600, 86400)))


def _tree_rss(rows: dict, pid: int) -> int:
    selected = {pid}
    while True:
        added = {p for p, (parent, _, _) in rows.items() if parent in selected} - selected
        if not added:
            return sum(rows[p][1] for p in selected if p in rows)
        selected.update(added)


def summarize(receipts: list[dict]) -> list[dict]:
    result = []
    for mode in ("cache", "incremental"):
        for case in CASES + ["execution_new"]:
            samples = [r for r in receipts if r["mode"] == mode and r["case"] == case]
            if not samples:
                continue
            row = {"mode": mode, "case": case, "n": len(samples)}
            for metric in (
                "wall_seconds",
                "cpu_seconds",
                "rss_bytes",
                "tree_rss_bytes",
                "daemon_cpu_seconds",
            ):
                values = [r[metric] for r in samples if r[metric] is not None]
                row[metric] = (
                    {"median": statistics.median(values), "min": min(values), "max": max(values)}
                    if len(values) == len(samples)
                    else None
                )
            result.append(row)
    return result


def validate(receipts: list[dict], cohorts: int, calibration_cohorts: Sequence[int] = ()) -> None:
    warm = set(range(1, cohorts + 1)) - set(calibration_cohorts)
    expected = {(c, m, case) for c in warm for m in ("cache", "incremental") for case in CASES}
    expected |= {(c, "cache", "execution_new") for c in warm}
    samples = [r for r in receipts if r["cohort"] in warm]
    actual = {(r["cohort"], r["mode"], r["case"]) for r in samples}
    if len(warm) < 3 or actual != expected or len(samples) != len(expected):
        raise ValueError("three or more complete cohorts are required")
    for r in receipts:
        if r["cohort"] in calibration_cohorts:
            continue
        if r["exit_code"] or r["tests_passed"] != 1:
            raise ValueError(f"focused test did not pass exactly once: {r['case']}")
        if not any(c["kind"] == "test" for c in r["workspace_compilers"]):
            raise ValueError(f"test compilation missing: {r['case']}")


def _require_disk(directory: Path, minimum: int) -> None:
    free = shutil.disk_usage(directory).free
    if free < minimum:
        raise SystemExit(
            f"benchmark disk budget: {free / 1024**3:.1f} GiB free; "
            f"the repository reserve is {minimum / 1024**3:.0f} GiB. "
            "Experiment receipts and state are retained; resume after space is available."
        )


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
    directory = os.open(output, os.O_RDONLY)
    try:
        try:
            fcntl.flock(directory, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            parser.error(f"measurement already running in {output}")
        # Only the Unix socket needs a short, temporary pathname.
        with tempfile.TemporaryDirectory(prefix="lf921-", dir="/tmp") as short:
            _measure(output, args.cohorts, args.resume, Path(short) / "cache.sock")
    finally:
        os.close(directory)


def _measure(output: Path, cohorts: int, resume: bool, socket: Path) -> None:
    state = output / "configuration.json"
    receipts_path = output / "receipts.json"
    if state.exists() and not resume:
        raise SystemExit("output already exists; use --resume")
    if resume and not state.exists():
        raise SystemExit(f"no measurement configuration to resume in {output}")
    if resume:
        config = json.loads(state.read_text())
        passed = output / "passed.json"
        if passed.exists() or not Path(config["temporary"]).is_dir():
            try:
                validate(
                    json.loads(receipts_path.read_text()),
                    config["cohorts"],
                    config.get("calibration_cohorts", []),
                )
            except (FileNotFoundError, ValueError) as error:
                if passed.exists():
                    raise
                raise SystemExit(
                    f"incomplete measurement; cache and worktrees missing: {config['temporary']}"
                ) from error
            if not passed.exists():
                _write(passed, {"passed": True})
            print(f"Measurement already complete: {output}")
            return
    reserve = load_policy().minimum_free_disk_bytes
    _require_disk(output, reserve)
    cache, cargo_env = configured_cache()
    if not cache or not cache.exists():
        raise SystemExit("run scripts/build_cache.py install first")
    env = {
        k: v
        for k, v in os.environ.items()
        if not k.startswith(("LF_", "SCCACHE_", "CARGO_"))
        and k not in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS")
    }
    env.update(cargo_env)
    revision = _run(["git", "rev-parse", "HEAD"], ROOT, env)
    if resume:
        cohorts = max(config["cohorts"], cohorts)
        config["cohorts"] = cohorts
        revision = config["revision"]
        config.setdefault("drivers", []).append(
            hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        )
        root = Path(config["temporary"])
        if config["cache_sha256"] != hashlib.sha256(cache.read_bytes()).hexdigest():
            raise ValueError("resume cache binary differs")
        _write(state, config)
    else:
        root = Path(tempfile.mkdtemp(prefix="worktrees-", dir=output))
        config = {
            "revision": revision,
            "drivers": [hashlib.sha256(Path(__file__).read_bytes()).hexdigest()],
            "cohorts": cohorts,
            "temporary": str(root),
            "cache": str(cache),
            "cache_sha256": hashlib.sha256(cache.read_bytes()).hexdigest(),
            "rustc": _run(["rustc", "-Vv"], ROOT, env),
            "cpus": os.cpu_count(),
            "machine": list(os.uname()),
            "jobs": tomlkit.loads(cargo_configuration().read_text()).get("build", {}).get("jobs"),
            "profiles": tomlkit.loads((ROOT / "Cargo.toml").read_text())
            .unwrap()
            .get("profile", {}),
            "filters": FILTERS,
            "created": time.time(),
            "trees": {},
        }
        _write(state, config)
    # Git invokes the registration hook while adding the measured worktrees.
    # Give the hook the same private inventory and socket as the compilers.
    env.update(
        SCCACHE_DIR=str(root / "cache"),
        SCCACHE_SERVER_UDS=str(socket),
        SCCACHE_IDLE_TIMEOUT="0",
        SCCACHE_CONF=str(root / "loopflow/sccache.json"),
    )
    receipts = json.loads(receipts_path.read_text()) if receipts_path.exists() else []
    # Register every measured root before starting the private daemon. Creating a
    # worktree is timed separately and included in its first sample's wall time.
    specs = [(0, mode, "seed") for mode in ("cache", "incremental")] + [
        (c, m, "mixed") for c in range(1, cohorts + 1) for m in ("cache", "incremental")
    ]
    specs += [(c, "cache", "execution") for c in range(1, cohorts + 1)]
    if resume:
        for cohort, mode, group in specs:
            key = f"{cohort}_{mode}_{group}"
            item = config["trees"].get(key)
            if item is None:
                continue
            if cohort in config.get("calibration_cohorts", []):
                shutil.rmtree(Path(item["path"]) / "target", ignore_errors=True)
                continue
            cases = (
                ["seed_app", "seed_execution"]
                if group == "seed"
                else ["execution_new"]
                if group == "execution"
                else CASES
            )
            rows = [
                r
                for r in receipts
                if r["cohort"] == cohort and r["mode"] == mode and r["case"] in cases
            ]
            if {r["case"] for r in rows if r["exit_code"] == 0 and r["tests_passed"] == 1} == set(
                cases
            ):
                continue
            tree = Path(item["path"])
            if not (tree / "target").exists() and not rows:
                continue
            # A partial compilation can populate both caches and incremental state.
            # Restart that group with distinct edits; keep its earlier raw attempts.
            attempt = item.get("attempt", 0)
            archive = output / "abandoned-attempts" / f"{key}_{attempt}"
            archive.mkdir(parents=True, exist_ok=True)
            _write(archive / "receipts.json", rows)
            for case in cases:
                for path in output.glob(f"{cohort}_{mode}_{case}.*"):
                    path.rename(archive / path.name)
                for path in output.glob(f"{cohort}_{mode}_{case}_compilers.jsonl"):
                    path.rename(archive / path.name)
            receipts = [r for r in receipts if r not in rows]
            _write(receipts_path, receipts)
            shutil.rmtree(tree / "target", ignore_errors=True)
            _run(["git", "restore", "--source=HEAD", "--worktree", "rust"], tree, env)
            item["attempt"] = attempt + 1
            _write(state, config)
    for cohort, mode, group in specs:
        key = f"{cohort}_{mode}_{group}"
        if key in config["trees"]:
            continue
        tree = root / key
        start = time.monotonic()
        _run(
            ["git", "worktree", "add", "--detach", str(tree), revision],
            ROOT,
            {**env, "XDG_CACHE_HOME": str(root)},
        )
        config["trees"][key] = {"path": str(tree), "setup_seconds": time.monotonic() - start}
        _write(state, config)
    refresh(ROOT, Path(env["SCCACHE_CONF"]))
    # cc-rs recognizes this basename and also routes native compiles through it.
    wrapper = root / "sccache"
    wrapper.write_text(
        f"#!{sys.executable}\n" + Path(__file__).with_name("compiler_receipt.py").read_text()
    )
    wrapper.chmod(0o755)
    env.update(
        RUSTC_WRAPPER=str(wrapper),
        LOOPFLOW_REBUILD_CACHE=str(cache),
        LOOPFLOW_REBUILD_ALL="1",
    )
    # The private daemon reads the same inventory and Machine environment as Cargo.
    # Its lifetime is bounded by this command, including failures and interruptions.
    try:
        _run([str(cache), "--start-server"], ROOT, env)
        server_pid = daemon_pid(env["SCCACHE_SERVER_UDS"])
        if server_pid is None:
            raise RuntimeError("private cache daemon exited at startup")

        def stats() -> dict:
            return json.loads(
                _run([str(cache), "--show-stats", "--stats-format", "json"], ROOT, env)
            )

        def sample(cohort: int, mode: str, group: str, case: str, owner: str, first: bool) -> None:
            if any((r["cohort"], r["mode"], r["case"]) == (cohort, mode, case) for r in receipts):
                return
            item = config["trees"][f"{cohort}_{mode}_{group}"]
            tree = Path(item["path"])
            stem = f"{cohort}_{mode}_{case}"
            compiler_log = output / f"{stem}_compilers.jsonl"
            compiler_log.unlink(missing_ok=True)
            selected_env = {
                **env,
                "LOOPFLOW_REBUILD_RECEIPTS": str(compiler_log),
                "CARGO_TARGET_DIR": str(tree / "target"),
                "CARGO_INCREMENTAL": "1" if mode == "incremental" else "0",
            }
            _require_disk(root, reserve)
            start = time.monotonic()
            edit_suffix = f"{stem}_attempt_{item.get('attempt', 0)}"
            edit = _edit(tree, owner, edit_suffix) if cohort else None
            before, load_before = stats(), os.getloadavg()
            daemon_before = _processes().get(server_pid)
            package, test_filter = FILTERS[owner]
            command = [
                "cargo",
                "test",
                "--locked",
                "-p",
                package,
                "--lib",
                test_filter,
                "--timings",
                "--message-format=json",
                "--",
                "--exact",
            ]
            peak = 0
            memory = []
            with (
                (output / f"{stem}.jsonl").open("w") as stdout,
                (output / f"{stem}.log").open("w") as stderr,
            ):
                child = subprocess.Popen(
                    ["/usr/bin/time", "-l" if sys.platform == "darwin" else "-v", *command],
                    cwd=tree,
                    env=selected_env,
                    stdout=stdout,
                    stderr=stderr,
                    stdin=subprocess.DEVNULL,
                    start_new_session=True,
                )
                try:
                    while child.poll() is None:
                        _require_disk(root, reserve)
                        rss = _tree_rss(_processes(), child.pid)
                        peak = max(peak, rss)
                        memory.append([round(time.monotonic() - start, 3), rss])
                        try:
                            child.wait(timeout=1)
                        except subprocess.TimeoutExpired:
                            pass
                finally:
                    if child.poll() is None:
                        os.killpg(child.pid, 15)
                        child.wait()
            elapsed = time.monotonic() - start
            after, load_after = stats(), os.getloadavg()
            daemon_after = _processes().get(server_pid)
            log = (output / f"{stem}.log").read_text()
            raw = (output / f"{stem}.jsonl").read_text()
            compilers = [json.loads(line) for line in compiler_log.read_text().splitlines()]
            for compiler in compilers:
                overlapping = any(
                    other["pid"] != compiler["pid"]
                    and other["started"] < compiler["finished"]
                    and other["finished"] > compiler["started"]
                    for other in compilers
                )
                compiler["overlapped"] = overlapping
                if overlapping:
                    compiler["cache_hits"] = compiler["cache_misses"] = None
                    compiler["non_cacheable"] = None
            artifacts = [
                json.loads(line) for line in raw.splitlines() if line.startswith('{"reason":')
            ]
            timings = tree / "target/cargo-timings/cargo-timing.html"
            timing_text = timings.read_text() if timings.exists() else ""
            match = re.search(r"const UNIT_DATA = (\[.*?\]);", timing_text, re.S)
            units = json.loads(match[1]) if match else None
            receipt = {
                "cohort": cohort,
                "mode": mode,
                "case": case,
                "owner": owner,
                "tree": str(tree),
                "edit": edit,
                "edit_suffix": edit_suffix,
                "command": command,
                "exit_code": child.returncode,
                "tests_passed": sum(
                    int(n) for n in re.findall(r"test result: ok\. (\d+) passed", raw)
                ),
                "wall_seconds": elapsed + (item["setup_seconds"] if first else 0),
                "setup_seconds": item["setup_seconds"] if first else 0,
                **_timing(log),
                "tree_rss_bytes": peak,
                "memory_samples": memory,
                "daemon_cpu_seconds": _cpu(daemon_after[2]) - _cpu(daemon_before[2])
                if daemon_before and daemon_after
                else None,
                "load_before": load_before,
                "load_after": load_after,
                "stats_before": before,
                "stats_after": after,
                "rust_cache_hits": after["stats"]["cache_hits"]["counts"].get("Rust", 0)
                - before["stats"]["cache_hits"]["counts"].get("Rust", 0),
                "rust_cache_misses": after["stats"]["cache_misses"]["counts"].get("Rust", 0)
                - before["stats"]["cache_misses"]["counts"].get("Rust", 0),
                "non_cacheable": {
                    key: value - before["stats"]["not_cached"].get(key, 0)
                    for key, value in after["stats"]["not_cached"].items()
                    if value != before["stats"]["not_cached"].get(key, 0)
                },
                "workspace_compilers": [c for c in compilers if c["crate"].startswith("loopflow")],
                "cargo_units": units,
                "cargo_units_note": None if units is not None else "Cargo HTML unit data not found",
                "linked_tests": [
                    a["executable"]
                    for a in artifacts
                    if a.get("reason") == "compiler-artifact"
                    and a.get("executable")
                    and a["profile"]["test"]
                    and not a["fresh"]
                ],
            }
            receipts.append(receipt)
            _write(receipts_path, receipts)
            _write(output / "summary.json", summarize(receipts))
            print(
                f"{stem}: exit {child.returncode}, "
                f"{receipt['tests_passed']} tests passed, {elapsed:.1f}s",
                flush=True,
            )
            if child.returncode or receipt["tests_passed"] != 1:
                raise RuntimeError(f"focused test failed: {output / (stem + '.log')}")

        for cohort in range(cohorts + 1):
            if cohort in config.get("calibration_cohorts", []):
                continue
            modes = (
                ("cache", "incremental")
                if cohort == 0
                else (("cache", "incremental") if cohort % 2 else ("incremental", "cache"))
            )
            for mode in modes:
                group = "seed" if cohort == 0 else "mixed"
                cases = ["seed_app", "seed_execution"] if cohort == 0 else CASES
                for index, case in enumerate(cases):
                    owner = (
                        "app" if "app" in case and case != "execution_after_app" else "execution"
                    )
                    sample(cohort, mode, group, case, owner, index == 0)
                tree = Path(config["trees"][f"{cohort}_{mode}_{group}"]["path"])
                shutil.rmtree(tree / "target", ignore_errors=True)
            if cohort:
                sample(cohort, "cache", "execution", "execution_new", "execution", True)
                tree = Path(config["trees"][f"{cohort}_cache_execution"]["path"])
                shutil.rmtree(tree / "target", ignore_errors=True)
        validate(receipts, cohorts, config.get("calibration_cohorts", []))
        for item in config["trees"].values():
            tree = Path(item["path"])
            if tree.exists():
                _run(["git", "worktree", "remove", "--force", str(tree)], ROOT, env)
    finally:
        _run([str(cache), "--stop-server"], ROOT, env)
        print(f"Receipts: {output}; experiment state: {root}", flush=True)
    shutil.rmtree(root)
    _write(output / "passed.json", {"passed": True})


if __name__ == "__main__":
    main()

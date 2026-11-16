"""Measure serial edits through passing focused tests in linked worktrees."""

import argparse
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
from pathlib import Path

import tomlkit
from run import _timing

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from build_cache import cargo_configuration, configured_cache, inventory_path, refresh  # noqa: E402

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


def validate(receipts: list[dict], cohorts: int) -> None:
    expected = {
        (c, m, case)
        for c in range(1, cohorts + 1)
        for m in ("cache", "incremental")
        for case in CASES
    }
    expected |= {(c, "cache", "execution_new") for c in range(1, cohorts + 1)}
    samples = [r for r in receipts if r["cohort"] > 0]
    actual = {(r["cohort"], r["mode"], r["case"]) for r in samples}
    if cohorts < 3 or actual != expected or len(samples) != len(expected):
        raise ValueError("three or more complete cohorts are required")
    for r in receipts:
        if r["exit_code"] or r["tests_passed"] != 1:
            raise ValueError(f"focused test did not pass exactly once: {r['case']}")
        if not any(c["kind"] == "test" for c in r["workspace_compilers"]):
            raise ValueError(f"test compilation missing: {r['case']}")


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
    state = output / "configuration.json"
    receipts_path = output / "receipts.json"
    if state.exists() and not args.resume:
        parser.error("output already exists; use --resume")
    cache, cargo_env = configured_cache()
    if not cache or not cache.exists():
        parser.error("run scripts/build_cache.py install first")
    env = {
        k: v
        for k, v in os.environ.items()
        if not k.startswith(("LF_", "SCCACHE_", "CARGO_"))
        and k not in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS")
    }
    env.update(cargo_env)
    revision = _run(["git", "rev-parse", "HEAD"], ROOT, env)
    if args.resume:
        config = json.loads(state.read_text())
        if config["revision"] != revision or config["cohorts"] != args.cohorts:
            raise ValueError("resume revision/cohort count differs")
        root = Path(config["temporary"])
        if config["cache_sha256"] != hashlib.sha256(cache.read_bytes()).hexdigest():
            raise ValueError("resume cache binary differs")
    else:
        root = Path(tempfile.mkdtemp(prefix="lf921-", dir="/tmp")).resolve()
        config = {
            "revision": revision,
            "cohorts": args.cohorts,
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
    receipts = json.loads(receipts_path.read_text()) if receipts_path.exists() else []
    # Register every measured root before starting the private daemon. Creating a
    # worktree is timed separately and included in its first sample's wall time.
    specs = [(0, "cache", "seed")] + [
        (c, m, "mixed") for c in range(1, args.cohorts + 1) for m in ("cache", "incremental")
    ]
    specs += [(c, "cache", "execution") for c in range(1, args.cohorts + 1)]
    if args.resume:
        for cohort, mode, group in specs:
            key = f"{cohort}_{mode}_{group}"
            item = config["trees"].get(key)
            if item is None:
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
        _run(["git", "worktree", "add", "--detach", str(tree), revision], ROOT, env)
        config["trees"][key] = {"path": str(tree), "setup_seconds": time.monotonic() - start}
        _write(state, config)
    refresh(ROOT, inventory_path())
    wrapper = root / "compiler-wrapper"
    wrapper.write_text(
        f"#!{sys.executable}\n" + Path(__file__).with_name("compiler_receipt.py").read_text()
    )
    wrapper.chmod(0o755)
    env.update(
        RUSTC_WRAPPER=str(wrapper),
        LOOPFLOW_REBUILD_CACHE=str(cache),
        LOOPFLOW_REBUILD_ALL="1",
        SCCACHE_DIR=str(root / "cache"),
        SCCACHE_SERVER_UDS=str(root / "cache.sock"),
    )
    # The private daemon reads the same inventory and Machine environment as Cargo.
    # Its lifetime is bounded by this command, including failures and interruptions.
    try:
        _run([str(cache), "--start-server"], ROOT, env)
        owners = _run(
            ["lsof", "-nP", "-a", "-U", "-c", "sccache", "-t", str(root / "cache.sock")], ROOT, env
        ).split()
        daemon_pid = next(
            int(p) for p in owners if _run(["ps", "-p", p, "-o", "comm="], ROOT, env) == str(cache)
        )

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
                "CARGO_INCREMENTAL": "1" if mode == "incremental" else "0",
            }
            start = time.monotonic()
            edit_suffix = f"{stem}_attempt_{item.get('attempt', 0)}"
            edit = _edit(tree, owner, edit_suffix) if cohort else None
            before, load_before = stats(), os.getloadavg()
            daemon_before = _processes().get(daemon_pid)
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
            daemon_after = _processes().get(daemon_pid)
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

        for cohort in range(args.cohorts + 1):
            modes = (
                ("cache",)
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
        validate(receipts, args.cohorts)
        _write(output / "passed.json", {"passed": True})
        for item in config["trees"].values():
            tree = Path(item["path"])
            if tree.exists():
                _run(["git", "worktree", "remove", "--force", str(tree)], ROOT, env)
    finally:
        _run([str(cache), "--stop-server"], ROOT, env)
        print(f"Receipts: {output}; experiment state: {root}", flush=True)
    if (output / "passed.json").exists():
        shutil.rmtree(root)
        refresh(ROOT, inventory_path())


if __name__ == "__main__":
    main()

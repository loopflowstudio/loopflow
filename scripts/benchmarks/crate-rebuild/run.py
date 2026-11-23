"""Compare serial CLI builds in disposable source copies with a private cache."""

import argparse
import hashlib
import json
import os
import re
import shutil
import statistics
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Callable

from compiler_receipt import install_wrapper

ROOT = Path(__file__).resolve().parents[3]
CASES = (
    "warm_root",
    "command_help",
    "execution",
    "task_record",
    "harness_format",
    "harness_test",
)
# Edit case → the libraries below the application that must recompile, where they exist.
_DOWNSTREAM = {
    "warm_root": (),
    "command_help": (),
    "execution": ("loopflow_execution",),
    "task_record": ("loopflow_storage", "loopflow_execution"),
    "harness_format": ("loopflow_harness", "loopflow_execution"),
}


def _libraries(tree: Path) -> list[str]:
    """Workspace libraries below the application in this revision."""
    return sorted(
        path.name.replace("-", "_")
        for path in (tree / "rust").glob("loopflow-*")
        if path.name != "loopflow-test-support" and (path / "Cargo.toml").exists()
    )


def _decoder_owner(tree: Path) -> str:
    if (tree / "rust/loopflow-harness").is_dir():
        return "loopflow-harness"
    return "loopflow-execution"


def _command(tree: Path, case: str) -> list[str]:
    if case == "harness_test":
        target = ["cargo", "test", "--locked", "-p", _decoder_owner(tree), "--lib", "--no-run"]
    else:
        target = ["cargo", "build", "--locked", "-p", "loopflow", "--bin", "lf"]
    return [*target, "--jobs", "1", "--message-format=json"]


def _verify_catalog_refresh(
    tree: Path, output: Path, run: Callable[[list[str], Path], str]
) -> None:
    """Exercise Cargo's real reference watches after the timed builds finish."""
    slug = "loo_650_catalog_probe"
    receipts = []

    def build(case: str) -> dict:
        raw = run(
            [
                "cargo",
                "build",
                "--offline",
                "-p",
                "loopflow-storage",
                "--jobs",
                "1",
                "--message-format=json",
            ],
            tree,
        )
        (output / f"catalog_{case}.jsonl").write_text(raw)
        events = [json.loads(line) for line in raw.splitlines() if line.startswith("{")]
        directories = {
            event["out_dir"]
            for event in events
            if event.get("reason") == "build-script-executed"
            and "/loopflow-storage#" in event["package_id"]
        }
        if len(directories) != 1:
            raise RuntimeError(f"storage catalogue output missing in {case}")
        directory = Path(directories.pop())
        catalog = re.findall(
            r'version: "([^"]+)".*merged: (true|false)',
            (directory / "migrations.rs").read_text(),
        )
        receipt = {
            "case": case,
            "catalog": catalog,
            "schema_sha256": hashlib.sha256(
                (directory / "canonical_schema.json").read_bytes()
            ).hexdigest(),
        }
        receipts.append(receipt)
        (output / "catalog-refresh.json").write_text(json.dumps(receipts, indent=2) + "\n")
        return receipt

    before = build("before")
    source = tree / "rust/loopflow-storage/src/migrations" / f"{slug}.sql"
    source.write_text("SELECT 1;\n")
    run(["git", "add", str(source.relative_to(tree))], tree)
    run(
        [
            "git",
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.com",
            "commit",
            "--quiet",
            "-m",
            "catalogue watch fixture",
        ],
        tree,
    )
    if dict(build("unmerged")["catalog"]).get(slug) != "false":
        raise RuntimeError("a branch migration was not recorded as unmerged")
    run(["git", "update-ref", "refs/remotes/origin/main", "HEAD"], tree)
    merged = build("main_advanced")
    if dict(merged["catalog"]).get(slug) != "true":
        raise RuntimeError("origin/main-only advancement did not refresh the catalogue")
    run(["git", "pack-refs", "--all"], tree)
    if build("packed")["catalog"] != merged["catalog"]:
        raise RuntimeError("packing references changed migration order")
    run(["git", "update-ref", "refs/remotes/origin/main", "HEAD~1"], tree)
    if dict(build("loose_after_packed")["catalog"]).get(slug) != "false":
        raise RuntimeError("a new loose reference did not refresh the catalogue")
    # A packaged source tree has the complete order and no Git directory.
    (tree / "rust/loopflow-storage/migration-order.txt").write_text(
        "".join(
            f"{name}\n" for name, _ in merged["catalog"] if re.fullmatch(r"[a-z][a-z0-9_]*", name)
        )
    )
    shutil.rmtree(tree / ".git")
    if build("source_package")["catalog"] != merged["catalog"]:
        raise RuntimeError("the source package changed the catalogue")
    if any(receipt["schema_sha256"] != before["schema_sha256"] for receipt in receipts):
        raise RuntimeError("a no-op migration or source move changed the product schema")


def _edit(tree: Path, case: str, cohort: int) -> Path:
    if case == "command_help":
        paths = [tree / "rust/loopflow/src/lf/mod.rs"]
        old = 'about = "Open Loopflow or run its CLI"'
    elif case == "execution":
        paths = [
            p
            for p in (tree / "rust").glob("*/src/ops/task/**/*.rs")
            if "pub enum ShipStart" in p.read_text()
        ]
        old = '"no Task exists for {issue:?}"'
    elif case in ("harness_format", "harness_test"):
        # Two messages in one decoder: the second edit follows a built test binary.
        paths = list((tree / "rust").glob("*/src/**/claude_decode.rs"))
        old = (
            '"Claude result changed conversation"'
            if case == "harness_format"
            else '"Claude turn failed"'
        )
    else:
        paths = [
            p
            for p in (tree / "rust").glob("*/src/**/task/mod.rs")
            if "pub enum TaskDataError" in p.read_text()
        ]
        old = '#[error("invalid Task: {0}")]'
    if len(paths) != 1 or paths[0].read_text().count(old) != 1:
        raise RuntimeError(f"cannot locate {case} semantic edit")
    source = paths[0]
    text = source.read_text()
    # Distinct edits across cohorts prevent retrieval of an earlier edited rlib.
    position = old.rfind('"')
    replacement = old[:position] + f" (rebuild cohort {cohort})" + old[position:]
    source.write_text(text.replace(old, replacement))
    return source


def _timing(log: str) -> dict:
    if os.uname().sysname == "Darwin":
        cpu = re.search(r"([\d.]+) real\s+([\d.]+) user\s+([\d.]+) sys", log)
        rss = re.search(r"(\d+)\s+maximum resident set size", log)
        return {"cpu_seconds": float(cpu[2]) + float(cpu[3]), "rss_bytes": int(rss[1])}
    user = re.search(r"User time \(seconds\): ([\d.]+)", log)
    system = re.search(r"System time \(seconds\): ([\d.]+)", log)
    rss = re.search(r"Maximum resident set size \(kbytes\): (\d+)", log)
    return {"cpu_seconds": float(user[1]) + float(system[1]), "rss_bytes": int(rss[1]) * 1024}


def _summarize(receipts: list[dict]) -> list[dict]:
    rows = []
    for label in ("base", "candidate"):
        for case in CASES:
            samples = [r for r in receipts if r["label"] == label and r["case"] == case]
            if not samples:
                continue
            row = {"label": label, "case": case, "n": len(samples)}
            for metric in ("wall_seconds", "cpu_seconds", "rss_bytes"):
                values = [r[metric] for r in samples]
                row[metric] = {
                    "median": statistics.median(values),
                    "min": min(values),
                    "max": max(values),
                }
            rows.append(row)
    return rows


def _validate_closure(receipt: dict) -> None:
    case = receipt["case"]
    if case == "cold_seed":
        return
    libraries = set(receipt["libraries"])
    compilers = receipt["workspace_compilers"]
    if case == "harness_test":
        owner = receipt["owner"].replace("-", "_")
        # Cargo passes a unit-test target `--test` and no crate type; an
        # executable is never cached, so its invocation is the compilation.
        tests = [c for c in compilers if c["crate"] == owner and c["crate_type"] is None]
        if len(tests) != 1:
            raise RuntimeError(f"the owner's test binary did not compile: {compilers}")
        if owner == "loopflow_harness" and {c["crate"] for c in compilers} != {owner}:
            raise RuntimeError(f"a decoder test build left its crate: {compilers}")
        return
    expected = {"loopflow"} | (set(_DOWNSTREAM[case]) & libraries)
    actual = {
        c["crate"] for c in compilers if c["crate_type"] in ("lib", "rlib") and c["cache_misses"]
    }
    if case == "warm_root":
        hits = {c["crate"] for c in compilers if c["cache_hits"]}
        if hits != libraries:
            raise RuntimeError(f"wrong reusable library closure: {hits}")
    if actual != expected or receipt["rust_cache_misses"] != len(expected):
        raise RuntimeError(f"wrong compiler closure: expected {expected}, got {actual}")


def _catalog_complete(output: Path) -> bool:
    path = output / "catalog-refresh.json"
    if not path.exists():
        return False
    rows = json.loads(path.read_text())
    return (
        [row["case"] for row in rows]
        == ["before", "unmerged", "main_advanced", "packed", "loose_after_packed", "source_package"]
        and len({row["schema_sha256"] for row in rows}) == 1
        and rows[-1]["catalog"] == rows[2]["catalog"]
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--candidate", default="HEAD")
    parser.add_argument("--sccache", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=1, choices=[1])
    parser.add_argument("--cohorts", type=int, default=5)
    parser.add_argument(
        "--resume", action="store_true", help="Continue retained sources and receipts"
    )
    args = parser.parse_args()
    if args.cohorts < 1:
        parser.error("--cohorts must be positive")
    cache = args.sccache.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if (output / "receipts.json").exists() and not args.resume:
        parser.error("output already has receipts; choose a fresh directory")
    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("LF_", "SCCACHE_", "CARGO_", "LOOPFLOW_BUILD_"))
        and key
        not in (
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "RUSTFLAGS",
            "LOOPFLOW_RELEASE_TAG",
            "LOOPFLOW_MIGRATION_ORDER",
        )
    }
    env.update(
        CARGO_INCREMENTAL="0",
        SCCACHE_CLIENT_SIDE="1",
        SCCACHE_MAX_FRAME_LENGTH=str(1024 * 1024 * 1024),
        SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS="CARGO_MANIFEST_DIR,CARGO_MANIFEST_PATH,LOOPFLOW_BUILD_SOURCE_ROOT",
        LOOPFLOW_REBUILD_CACHE=str(cache),
    )
    receipts = json.loads((output / "receipts.json").read_text()) if args.resume else []

    for receipt in receipts:
        _validate_closure(receipt)

    def run(argv: list[str], cwd: Path = ROOT) -> str:
        result = subprocess.run(
            argv, cwd=cwd, env=env, capture_output=True, text=True, stdin=subprocess.DEVNULL
        )
        if result.returncode:
            raise RuntimeError(f"{argv!r} failed:\n{result.stdout}\n{result.stderr}")
        return result.stdout

    def stats() -> dict:
        return json.loads(run([str(cache), "--show-stats", "--stats-format", "json"]))["stats"]

    if not args.resume:
        (output / "configuration.json").write_text(
            json.dumps(
                {
                    "cache_binary": str(cache),
                    "cache_sha256": hashlib.sha256(cache.read_bytes()).hexdigest(),
                    "cache_version": run([str(cache), "--version"]).strip(),
                    "rustc": run(["rustc", "--version"]).strip(),
                    "machine": list(os.uname()),
                    "cpus": os.cpu_count(),
                    "jobs": args.jobs,
                    "cohorts": args.cohorts,
                    "client_side": True,
                    "max_frame_length": env["SCCACHE_MAX_FRAME_LENGTH"],
                    "instrumentation": (
                        "two cache-stat RPCs per workspace rustc invocation; "
                        "included in elapsed/client CPU"
                    ),
                    "memory": "largest timed process RSS; cache daemon excluded from CPU/RSS",
                },
                indent=2,
            )
            + "\n"
        )
    retained = json.loads((output / "frontiers.json").read_text()) if args.resume else []
    if args.resume:
        prior = json.loads((output / "configuration.json").read_text())
        if prior["rustc"] != run(["rustc", "--version"]).strip():
            raise RuntimeError("resume requires the same Rust compiler")
        if prior["cache_sha256"] != hashlib.sha256(cache.read_bytes()).hexdigest():
            raise RuntimeError("resume requires the same cache binary")
    temporary = (
        str(Path(retained[0]["tree"]).parent)
        if retained
        else tempfile.mkdtemp(prefix="lf650-build-", dir="/tmp")
    )
    root = Path(temporary).resolve()
    try:
        env.update(
            RUSTC_WRAPPER=str(install_wrapper(root)),
            SCCACHE_SERVER_UDS=str(root / "cache.sock"),
            SCCACHE_CONF=str(root / "sccache.toml"),
        )
        (root / "sccache.toml").write_text("")
        snapshots = {}
        for label, revision in (("base", args.base), ("candidate", args.candidate)):
            sha = run(["git", "rev-parse", "--verify", revision + "^{commit}"]).strip()
            trees = [root / f"{label}-{i}" for i in range(args.cohorts + 1)]
            if args.resume:
                rows = [row for row in retained if Path(row["tree"]).name.startswith(label + "-")]
                if len(rows) != len(trees) or any(row["revision"] != sha for row in rows):
                    raise RuntimeError("resume revisions or cohort count differ")
            else:
                for tree in trees:
                    run(
                        [
                            "git",
                            "clone",
                            "--shared",
                            "--no-checkout",
                            "--quiet",
                            str(ROOT),
                            str(tree),
                        ]
                    )
                    run(["git", "checkout", "--quiet", "--detach", sha], tree)
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
        if not args.resume:
            (output / "frontiers.json").write_text(
                json.dumps(
                    [
                        {
                            "tree": str(t),
                            "revision": sha,
                            "origin_main": run(["git", "rev-parse", "origin/main"], t).strip(),
                        }
                        for sha, trees in snapshots.values()
                        for t in trees
                    ],
                    indent=2,
                )
                + "\n"
            )
        inputs = {}
        for label, (_, trees) in snapshots.items():
            tree = trees[0]
            lock = (tree / "Cargo.lock").read_text()
            inputs[label] = {
                "registry_packages": sorted(
                    re.findall(
                        r'name = "([^\"]+)"\nversion = "([^\"]+)"\n'
                        r'source = "registry[^\n]+\nchecksum = "([^\"]+)"',
                        lock,
                    )
                ),
                "profiles": re.findall(
                    r"\[profile\.[^\]]+\]\n(.*?)(?=\n\[|\Z)",
                    (tree / "Cargo.toml").read_text(),
                    re.S,
                ),
            }
        (output / "inputs.json").write_text(json.dumps(inputs, indent=2) + "\n")
        if inputs["base"]["profiles"] != inputs["candidate"]["profiles"]:
            raise RuntimeError("baseline and candidate profiles differ")
        env["SCCACHE_BASEDIRS"] = os.pathsep.join(
            str(t) for _, trees in snapshots.values() for t in trees
        )
        # Seed each independent cache once; these calibration builds are not cohort samples.
        for cohort in range(args.cohorts + 1):
            order = ("base", "candidate") if cohort % 2 == 0 else ("candidate", "base")
            for label in order:
                sha, trees = snapshots[label]
                tree = trees[cohort]
                cases = ("cold_seed",) if cohort == 0 else CASES
                completed = {
                    r["case"] for r in receipts if r["label"] == label and r["cohort"] == cohort
                }
                identity_path = output / f"{cohort}_{label}_identity.json"
                if set(cases) <= completed and identity_path.exists():
                    if cohort != 0 or label != "candidate" or _catalog_complete(output):
                        continue
                env["SCCACHE_DIR"] = str(root / f"{label}-cache")
                try:
                    run(["nice", "-n", "10", str(cache), "--start-server"])
                    for case in cases:
                        receipt = next(
                            (
                                r
                                for r in receipts
                                if r["label"] == label
                                and r["cohort"] == cohort
                                and r["case"] == case
                            ),
                            None,
                        )
                        if receipt is None:
                            if case == "harness_test":
                                # Dev-dependencies and the first test binary are not the loop.
                                run(_command(tree, case), tree)
                            source = (
                                _edit(tree, case, cohort)
                                if case not in ("cold_seed", "warm_root")
                                else None
                            )
                            stem = f"{cohort}_{label}_{case}"
                            compiler_path = output / f"{stem}_compilers.jsonl"
                            env["LOOPFLOW_REBUILD_RECEIPTS"] = str(compiler_path)
                            before, load = stats(), os.getloadavg()
                            start = time.monotonic()
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
                                        *_command(tree, case),
                                    ],
                                    cwd=tree,
                                    env=env,
                                    stdout=stdout,
                                    stderr=stderr,
                                    stdin=subprocess.DEVNULL,
                                )
                            elapsed = time.monotonic() - start
                            env.pop("LOOPFLOW_REBUILD_RECEIPTS")
                            after = stats()
                            log = (output / f"{stem}.log").read_text()
                            if result.returncode:
                                raise RuntimeError(f"build failed: see {output / (stem + '.log')}")
                            artifacts = [
                                json.loads(line)
                                for line in (output / f"{stem}.jsonl").read_text().splitlines()
                                if line.startswith("{")
                            ]
                            compilers = [
                                json.loads(line) for line in compiler_path.read_text().splitlines()
                            ]
                            receipt = {
                                "label": label,
                                "revision": sha,
                                "cohort": cohort,
                                "case": case,
                                "exit_code": result.returncode,
                                "wall_seconds": round(elapsed, 3),
                                **_timing(log),
                                "load_before": load,
                                "load_after": os.getloadavg(),
                                "edited_source": str(source.relative_to(tree)) if source else None,
                                "libraries": _libraries(tree),
                                "owner": _decoder_owner(tree) if case == "harness_test" else None,
                                "workspace_compilers": compilers,
                                "nonfresh_libraries": sorted(
                                    {
                                        a["target"]["name"]
                                        for a in artifacts
                                        if a.get("reason") == "compiler-artifact"
                                        and not a["fresh"]
                                        and set(a["target"]["kind"])
                                        & {"lib", "rlib", "dylib", "cdylib", "staticlib"}
                                    }
                                ),
                                "stats_before": before,
                                "stats_after": after,
                            }
                            for key in ("cache_hits", "cache_misses"):
                                receipt["rust_" + key] = after[key]["counts"].get(
                                    "Rust", 0
                                ) - before[key]["counts"].get("Rust", 0)
                            for counter in (
                                "cache_write_errors",
                                "cache_read_errors",
                                "cache_timeouts",
                            ):
                                if after[counter] != before[counter]:
                                    raise RuntimeError(f"cache failure in {stem}: {counter}")
                            receipts.append(receipt)
                            (output / "receipts.json").write_text(
                                json.dumps(receipts, indent=2) + "\n"
                            )
                            (output / "summary.json").write_text(
                                json.dumps(_summarize(receipts), indent=2) + "\n"
                            )
                            print(
                                json.dumps(
                                    {
                                        k: receipt[k]
                                        for k in (
                                            "label",
                                            "cohort",
                                            "case",
                                            "wall_seconds",
                                            "workspace_compilers",
                                        )
                                    }
                                ),
                                flush=True,
                            )
                        _validate_closure(receipt)
                        run([str(tree / "target/debug/lf"), "--version"], tree)
                        if case == "warm_root":
                            paths = []
                            for crate in ("loopflow", *_libraries(tree)):
                                for artifact in (tree / "target/debug/deps").glob(
                                    f"lib{crate}-*.rlib"
                                ):
                                    data = artifact.read_bytes()
                                    paths.append(
                                        {
                                            "crate": crate,
                                            "seed_root_bytes": str(trees[0]).encode() in data,
                                            "current_root_bytes": str(tree).encode() in data,
                                        }
                                    )
                                    del data
                            (output / f"{cohort}_{label}_source_paths.json").write_text(
                                json.dumps(paths, indent=2) + "\n"
                            )

                    # Correct-checkout identity is checked after timing, without opening a store.
                    probe = tree / "rust/loopflow/src/bin/loo650_identity.rs"
                    initialize = (
                        "loopflow::build_info::initialize();"
                        if (tree / "rust/loopflow-execution").is_dir()
                        else ""
                    )
                    probe.write_text(
                        "fn main() { " + initialize + ' println!("{}|{}", '
                        "loopflow::build_info::source_root().unwrap().display(), "
                        "loopflow::build_info::source_revision()); }"
                    )
                    manifest = tree / "rust/loopflow/Cargo.toml"
                    original_manifest = run(
                        ["git", "show", sha + ":rust/loopflow/Cargo.toml"], ROOT
                    )
                    with manifest.open("a") as stream:
                        stream.write(
                            '\n[[bin]]\nname="loo650-identity"\npath="src/bin/loo650_identity.rs"\n'
                        )
                    identity = run(
                        [
                            "cargo",
                            "run",
                            "--locked",
                            "--quiet",
                            "-p",
                            "loopflow",
                            "--bin",
                            "loo650-identity",
                            "--jobs",
                            "1",
                        ],
                        tree,
                    ).strip()
                    if identity != f"{tree}|{sha}-dirty":
                        raise RuntimeError(f"wrong runtime identity: {identity}")
                    (output / f"{cohort}_{label}_identity.json").write_text(
                        json.dumps(
                            {
                                "root": str(tree),
                                "revision": sha,
                                "observed": identity,
                                "dirty": True,
                            }
                        )
                        + "\n"
                    )
                    probe.unlink()
                    manifest.write_text(original_manifest)
                    if cohort == 0 and label == "candidate":
                        _verify_catalog_refresh(tree, output, run)
                    # Recover disk between cohorts; every target belongs to exactly one source root.
                    shutil.rmtree(tree / "target")
                finally:
                    run([str(cache), "--stop-server"])
    except BaseException:
        print(f"Build sources, targets and caches retained at {temporary}", flush=True)
        raise
    else:
        shutil.rmtree(temporary)


if __name__ == "__main__":
    main()

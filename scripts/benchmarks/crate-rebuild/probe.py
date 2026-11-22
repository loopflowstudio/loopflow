"""Verify cross-root reuse of the real extracted crates with a private sccache server."""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[3]


def _verify_receipts(receipts: list[dict]) -> None:
    expected_hits = {
        "model_b",
        "definitions_b",
        "terminal_b",
        "storage_b",
        "storage_test_support_b",
        "new_root_after_refresh",
    }
    for receipt in receipts:
        case = receipt["case"]
        owner = None
        if case.startswith(("model_", "new_root_", "changed_model_")):
            owner = "loopflow_model"
        if case.startswith(("storage_", "changed_storage_")):
            owner = "loopflow_storage"
        if owner and receipt["libraries"] != [owner]:
            raise RuntimeError(f"owner dependencies were not warm: {receipt}")
        count = len(receipt["libraries"])
        expected = (count, 0) if case in expected_hits else (0, count)
        actual = receipt["cache_hits"], receipt["cache_misses"]
        if actual != expected:
            raise RuntimeError(f"unexpected cache result: {receipt}; expected {expected}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sccache", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    cache = args.sccache.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    (output / "passed.json").unlink(missing_ok=True)
    package = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]
    package_fields = "".join(
        f"{key}={json.dumps(package[key])}\n" for key in ("version", "edition", "license")
    )
    with tempfile.TemporaryDirectory(prefix="lf650-", dir="/tmp") as temporary:
        root = Path(temporary).resolve()
        trees = [root / name for name in ("a", "b")]
        for tree in trees:
            for package in (
                "loopflow-model",
                "loopflow-definitions",
                "loopflow-terminal",
                "loopflow-storage",
                "loopflow-test-support",
            ):
                shutil.copytree(ROOT / "rust" / package, tree / "rust" / package)
            shutil.copyfile(ROOT / "Cargo.lock", tree / "Cargo.lock")
            (tree / "Cargo.toml").write_text(
                '[workspace]\nresolver="2"\n'
                'members=["rust/loopflow-model", "rust/loopflow-definitions", '
                '"rust/loopflow-terminal", "rust/loopflow-storage", "rust/loopflow-test-support", '
                '"rust/model-dependencies", "rust/storage-dependencies", "path-fixture"]\n'
                "[workspace.package]\n"
                + package_fields
                + "[profile.dev]\ndebug=1\nincremental=false\n"
            )
            # Match the model's exact dependency features so its measured
            # invocation contains only the owner, including in an unlisted root.
            dependencies = tree / "rust/model-dependencies"
            (dependencies / "src").mkdir(parents=True)
            (dependencies / "Cargo.toml").write_text(
                (tree / "rust/loopflow-model/Cargo.toml")
                .read_text()
                .replace('name = "loopflow-model"', 'name = "model-dependencies"', 1)
            )
            (dependencies / "src/lib.rs").write_text("")
            dependencies = tree / "rust/storage-dependencies"
            (dependencies / "src").mkdir(parents=True)
            manifest = (tree / "rust/loopflow-storage/Cargo.toml").read_text()
            (dependencies / "Cargo.toml").write_text(
                manifest.replace(
                    'name = "loopflow-storage"', 'name = "storage-dependencies"', 1
                ).replace('links = "loopflow_storage"\n', "")
            )
            # Match both host/build and target/runtime dependency features.
            (dependencies / "src/lib.rs").write_text("")
            (dependencies / "build.rs").write_text("fn main() {}\n")
            fixture = tree / "path-fixture/src"
            fixture.mkdir(parents=True)
            (fixture.parent / "Cargo.toml").write_text(
                '[package]\nname="path-fixture"\nversion="0.1.0"\nedition="2021"\n'
            )
            (fixture / "lib.rs").write_text(
                'pub fn root() -> &\'static str { env!("CARGO_MANIFEST_DIR") }\n'
            )
            (fixture / "main.rs").write_text(
                'fn main() { println!("{}", path_fixture::root()); }\n'
            )
        env = {
            key: value
            for key, value in os.environ.items()
            if not key.startswith(("SCCACHE_", "CARGO_", "LF_"))
            and key not in ("RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER")
        }
        env.update(
            SCCACHE_SERVER_UDS=str(root / "cache.sock"),
            SCCACHE_DIR=str(root / "cache"),
            SCCACHE_CONF=str(root / "sccache.toml"),
            SCCACHE_BASEDIRS=os.pathsep.join(map(str, trees)),
            SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS=(
                "CARGO_MANIFEST_DIR,CARGO_MANIFEST_PATH,LOOPFLOW_BUILD_SOURCE_ROOT"
            ),
            CARGO_INCREMENTAL="0",
            SCCACHE_CLIENT_SIDE="1",
            # Client-side uploads include the compressed library. Storage and
            # application artifacts exceed the daemon's default 8 MiB frame.
            SCCACHE_MAX_FRAME_LENGTH=str(1024 * 1024 * 1024),
            RUSTC_WRAPPER=str(cache),
        )
        (root / "sccache.toml").write_text("")

        def run(command: list[str], cwd: Path = root) -> subprocess.CompletedProcess[str]:
            result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True)
            if result.returncode:
                raise RuntimeError(f"{command!r} failed:\n{result.stdout}\n{result.stderr}")
            return result

        def stats() -> dict:
            return json.loads(run([str(cache), "--show-stats", "--stats-format", "json"]).stdout)[
                "stats"
            ]

        (output / "configuration.json").write_text(
            json.dumps(
                {
                    "cache_binary": str(cache),
                    "cache_sha256": hashlib.sha256(cache.read_bytes()).hexdigest(),
                    "cache_version": run([str(cache), "--version"]).stdout.strip(),
                    "rustc": run(["rustc", "--version"]).stdout.strip(),
                    "initial_roots": list(map(str, trees)),
                    "verbatim_env_deps": env["SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS"],
                },
                indent=2,
            )
            + "\n"
        )
        receipts = []

        def build(label: str, tree: Path, package: str, features: str | None = None) -> None:
            before = stats()
            start = time.monotonic()
            result = run(
                [
                    "/usr/bin/time",
                    "-l" if os.uname().sysname == "Darwin" else "-v",
                    "nice",
                    "-n",
                    "10",
                    "cargo",
                    "build",
                    "--offline",
                    "--jobs",
                    "1",
                    "-p",
                    package,
                    "--message-format=json",
                    *(["--features", features] if features else []),
                ],
                tree,
            )
            after = stats()
            (output / f"{label}.jsonl").write_text(result.stdout)
            (output / f"{label}.log").write_text(result.stderr)
            receipt = {
                "case": label,
                "wall_seconds": round(time.monotonic() - start, 3),
                "stats_before": before,
                "stats_after": after,
            }
            for field in ("cache_hits", "cache_misses"):
                receipt[field] = after[field]["counts"].get("Rust", 0) - before[field][
                    "counts"
                ].get("Rust", 0)
            artifacts = [
                json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")
            ]
            receipt["libraries"] = sorted(
                {
                    item["target"]["name"]
                    for item in artifacts
                    if item.get("reason") == "compiler-artifact"
                    and not item["fresh"]
                    and "lib" in item["target"]["kind"]
                }
            )
            if package == "loopflow-storage":
                directories = {
                    item["out_dir"]
                    for item in artifacts
                    if item.get("reason") == "build-script-executed"
                    and "/loopflow-storage#" in item["package_id"]
                }
                if len(directories) != 1:
                    raise RuntimeError(f"storage catalogue output missing in {label}")
                directory = Path(directories.pop())
                receipt["fixture_sha256"] = hashlib.sha256(
                    (directory / "test-store.db").read_bytes()
                ).hexdigest()
                if label.endswith("_b"):
                    first = next(item for item in receipts if item["case"] == "storage_a")
                    if receipt["fixture_sha256"] != first["fixture_sha256"]:
                        raise RuntimeError(f"fixture bytes changed between roots: {receipt}")
            if package.replace("-", "_") not in receipt["libraries"]:
                raise RuntimeError(f"owner did not compile in {label}")
            receipts.append(receipt)
            print(json.dumps(receipt), flush=True)
            _verify_receipts([receipt])

        # Exercise the real build-script ref watches, including the shared Git
        # directory, while each linked checkout keeps its own Cargo target.
        repository = root / "repository"
        run(["git", "clone", "--shared", "--no-checkout", str(ROOT), str(repository)])
        shutil.move(repository / ".git", trees[0] / ".git")
        shutil.rmtree(repository)
        run(["git", "add", "--all"], trees[0])
        run(
            [
                "git",
                "-c",
                "user.name=Cache fixture",
                "-c",
                "user.email=fixture@example.com",
                "commit",
                "-qm",
                "source snapshot",
            ],
            trees[0],
        )
        main_revision = run(["git", "rev-parse", "origin/main"], ROOT).stdout.strip()
        run(["git", "update-ref", "refs/remotes/origin/main", main_revision], trees[0])
        shutil.rmtree(trees[1])
        run(["git", "worktree", "add", "-qb", "cache-b", str(trees[1])], trees[0])

        try:
            run(["nice", "-n", "10", str(cache), "--start-server"])
            for label, tree in zip(("a", "b"), trees):
                # Isolate the model invocation from its dependencies' cache counters.
                result = run(
                    [
                        "nice",
                        "-n",
                        "10",
                        "cargo",
                        "build",
                        "--offline",
                        "--jobs",
                        "1",
                        "-p",
                        "model-dependencies",
                    ],
                    tree,
                )
                (output / f"dependencies_{label}.log").write_text(result.stderr)
                build(f"model_{label}", tree, "loopflow-model")
                build(f"definitions_{label}", tree, "loopflow-definitions")
                build(f"terminal_{label}", tree, "loopflow-terminal")
                result = run(
                    ["cargo", "build", "--offline", "--jobs", "1", "-p", "storage-dependencies"],
                    tree,
                )
                (output / f"storage_dependencies_{label}.log").write_text(result.stderr)
                build(f"storage_{label}", tree, "loopflow-storage")
                for package in (
                    "loopflow_model",
                    "loopflow_definitions",
                    "loopflow_terminal",
                    "loopflow_storage",
                ):
                    library = tree / "target/debug" / f"lib{package}.rlib"
                    shutil.copyfile(library, output / f"{package}_{label}.rlib")
                    paths = run(["strings", str(library)]).stdout
                    (output / f"{package}_{label}_source_paths.txt").write_text(
                        "\n".join(line for line in paths.splitlines() if str(root) in line) + "\n"
                    )
                build(
                    f"storage_test_support_{label}",
                    tree,
                    "loopflow-storage",
                    "test-support",
                )
                build(f"path_{label}", tree, "path-fixture")
                actual = run([str(tree / "target/debug/path-fixture")], tree).stdout.strip()
                if actual != str(tree / "path-fixture"):
                    raise RuntimeError(f"wrong checkout identity: {actual}")
            changed = trees[1] / "rust/loopflow-storage/src/work/task/mod.rs"
            changed.write_text(
                changed.read_text().replace(
                    '#[error("invalid Task: {0}")]', '#[error("invalid Task: {0} (cache probe)")]'
                )
            )
            build("changed_storage_b", trees[1], "loopflow-storage")
            if receipts[-1]["libraries"] != ["loopflow_storage"]:
                raise RuntimeError(f"Task edit rebuilt outside storage: {receipts[-1]}")
            # Roots are server-startup state. A newly added root misses until
            # our own idle server is refreshed; the same cache then serves it.
            third = root / "c"
            shutil.copytree(trees[0], third, ignore=shutil.ignore_patterns("target", ".git"))
            env["SCCACHE_BASEDIRS"] = os.pathsep.join(map(str, [trees[0], third]))
            run(
                [
                    "nice",
                    "-n",
                    "10",
                    "cargo",
                    "build",
                    "--offline",
                    "--jobs",
                    "1",
                    "-p",
                    "model-dependencies",
                ],
                third,
            )
            build("new_root_before_refresh", third, "loopflow-model")
            run([str(cache), "--stop-server"])
            run(["nice", "-n", "10", str(cache), "--start-server"])
            # Compare a fresh target after refresh. Dependencies compiled before
            # normalization can carry different debug paths into their hashes.
            shutil.rmtree(third / "target")
            run(
                [
                    "nice",
                    "-n",
                    "10",
                    "cargo",
                    "build",
                    "--offline",
                    "--jobs",
                    "1",
                    "-p",
                    "model-dependencies",
                ],
                third,
            )
            build("new_root_after_refresh", third, "loopflow-model")
            # Restore B to the normalized roots before checking changed content.
            run([str(cache), "--stop-server"])
            env["SCCACHE_BASEDIRS"] = os.pathsep.join(map(str, trees))
            run(["nice", "-n", "10", str(cache), "--start-server"])
            changed = trees[1] / "rust/loopflow-model/src/program_status.rs"
            changed.write_text(
                changed.read_text().replace("MAX_RECORDS: usize = 64", "MAX_RECORDS: usize = 63")
            )
            build("changed_model_b", trees[1], "loopflow-model")
        finally:
            (output / "receipts.json").write_text(json.dumps(receipts, indent=2) + "\n")
            run([str(cache), "--stop-server"])
        (output / "passed.json").write_text('{"passed": true}\n')


if __name__ == "__main__":
    main()

"""Probe installed sccache with a real, dependency-free Loopflow source file."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def main() -> None:
    source = Path(__file__).resolve().parents[1] / "rust/loopflow/src/command.rs"
    with tempfile.TemporaryDirectory(prefix="lf650-", dir="/tmp") as temporary:
        root = Path(temporary).resolve()
        trees = [root / name for name in ("a", "b")]
        for tree in trees:
            (tree / "src").mkdir(parents=True)
            shutil.copyfile(source, tree / "src/lib.rs")
            (tree / "Cargo.toml").write_text(
                '[package]\nname="lf650-probe"\nversion="0.1.0"\nedition="2021"\n'
                '[profile.dev]\ndebug=1\nincremental=false\n'
            )
        env = {
            key: value for key, value in os.environ.items()
            if not key.startswith(("SCCACHE_", "CARGO_", "LF_"))
            and key not in ("RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER")
        }
        env.update(
            SCCACHE_SERVER_UDS=str(root / "cache.sock"),
            SCCACHE_DIR=str(root / "cache"),
            SCCACHE_CONF=str(root / "sccache.toml"),
            SCCACHE_BASEDIRS=os.pathsep.join(map(str, trees)),
            CARGO_INCREMENTAL="0",
            RUSTC_WRAPPER=shutil.which("sccache"),
        )
        (root / "sccache.toml").write_text("")

        def run(args: list[str], cwd: Path = root) -> str:
            result = subprocess.run(args, cwd=cwd, env=env, capture_output=True, text=True)
            if result.returncode:
                raise RuntimeError(result.stdout + result.stderr)
            return result.stdout

        try:
            run(["nice", "-n", "10", "sccache", "--start-server"])
            for label, tree in (("cold_a", trees[0]), ("replay_a", trees[0]), ("cross_b", trees[1])):
                shutil.rmtree(tree / "target", ignore_errors=True)
                before = json.loads(run(["sccache", "--show-stats", "--stats-format", "json"]))
                started = time.monotonic()
                run(["nice", "-n", "10", "cargo", "build", "--offline", "--jobs", "1"], tree)
                after = json.loads(run(["sccache", "--show-stats", "--stats-format", "json"]))
                def delta(name: str) -> int:
                    return (after["stats"][name]["counts"].get("Rust", 0)
                            - before["stats"][name]["counts"].get("Rust", 0))

                print(json.dumps({"case": label, "wall_seconds": round(time.monotonic() - started, 3),
                                  "rust_hits": delta("cache_hits"), "rust_misses": delta("cache_misses")}))
        finally:
            run(["sccache", "--stop-server"])


if __name__ == "__main__":
    main()

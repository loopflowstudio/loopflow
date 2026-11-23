"""Workspace cache counters and optional compiler intervals, installed as `sccache`."""

import json
import os
import subprocess
import sys
import time
from pathlib import Path


def install_wrapper(directory: Path) -> Path:
    # cc-rs routes native builds through wrappers named sccache.
    wrapper = directory / "sccache"
    wrapper.write_text(f"#!{sys.executable}\n" + Path(__file__).read_text())
    wrapper.chmod(0o755)
    return wrapper


def main() -> None:
    cache = os.environ["LOOPFLOW_REBUILD_CACHE"]
    arguments = sys.argv[1:]
    name = None
    if "--crate-name" in arguments:
        name = arguments[arguments.index("--crate-name") + 1]
    receipt = os.environ.get("LOOPFLOW_REBUILD_RECEIPTS")
    workspace = name == "lf" or (name or "").startswith("loopflow")
    if not receipt or (not workspace and os.environ.get("LOOPFLOW_REBUILD_ALL") != "1"):
        os.execv(cache, [cache, *arguments])

    def stats() -> dict:
        result = subprocess.run(
            [cache, "--show-stats", "--stats-format", "json"],
            check=True,
            capture_output=True,
            text=True,
        )
        return json.loads(result.stdout)["stats"]

    started = time.time()
    before = stats() if workspace else None
    start = time.monotonic()
    result = subprocess.run([cache, *arguments])
    after = stats() if workspace else None
    item = {
        "crate": name or "unknown",
        "pid": os.getpid(),
        "started": started,
        "finished": time.time(),
        "kind": "test" if "--test" in arguments else "compile",
        "package": os.environ.get("CARGO_PKG_NAME"),
        "crate_type": arguments[arguments.index("--crate-type") + 1]
        if "--crate-type" in arguments
        else None,
        "seconds": round(time.monotonic() - start, 3),
        "exit_code": result.returncode,
    }
    for key in ("cache_hits", "cache_misses"):
        item[key] = (
            after[key]["counts"].get("Rust", 0) - before[key]["counts"].get("Rust", 0)
            if before is not None
            else None
        )
    item["non_cacheable"] = (
        {
            key: count - before["not_cached"].get(key, 0)
            for key, count in after["not_cached"].items()
            if count != before["not_cached"].get(key, 0)
        }
        if before is not None
        else None
    )
    with Path(receipt).open("a") as stream:
        stream.write(json.dumps(item) + "\n")
    sys.exit(result.returncode)


if __name__ == "__main__":
    main()

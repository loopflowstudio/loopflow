"""Register a second Loopflow worktree while the first compiles a focused test."""

import argparse
import json
import os
import signal
import subprocess
import sys
import tempfile
import time
from contextlib import ExitStack
from pathlib import Path

from test_loop import FILTERS, ROOT, _require_disk, _run

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
import build_cache  # noqa: E402
from resource_envelope import load_policy  # noqa: E402


def _execution_compiler(group: int) -> int | None:
    rows = subprocess.check_output(["ps", "-ww", "-axo", "pgid=,pid=,command="], text=True)
    for row in rows.splitlines():
        fields = row.split(None, 3)
        if len(fields) != 4 or fields[0] != str(group) or Path(fields[2]).name != "rustc":
            continue
        args = fields[3].replace("--emit=", "--emit ").split()
        if "--crate-name" not in args or "--emit" not in args:
            continue
        name = args[args.index("--crate-name") + 1]
        emits = args[args.index("--emit") + 1].split(",")
        if name == "loopflow_execution" and "link" in emits:
            return int(fields[1])
    return None


def _wait_for_exit(pid: int) -> bool:
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return True
        time.sleep(0.1)
    return False


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    output = parser.parse_args().output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    receipt = output / "stop.json"
    receipt.unlink(missing_ok=True)
    wrapper, settings = build_cache.configured_cache()
    if not wrapper or not wrapper.is_file():
        parser.error("run build_cache.py install first")
    reserve = load_policy().minimum_free_disk_bytes
    with (
        tempfile.TemporaryDirectory(dir="/tmp", prefix="lf-stop-") as temporary,
        ExitStack() as stack,
    ):
        root = Path(temporary).resolve()
        _require_disk(root, reserve)
        a, b = root / "building", root / "registering"
        env = {
            k: v
            for k, v in os.environ.items()
            if not k.startswith(("LF_", "CARGO_", "SCCACHE_"))
            and k not in ("RUSTFLAGS", "RUSTC_WORKSPACE_WRAPPER")
        }
        env.update(
            settings,
            RUSTC_WRAPPER=str(wrapper),
            CARGO_TARGET_DIR=str(a / "target"),
            CARGO_INCREMENTAL="0",
            SCCACHE_DIR=str(root / "cache"),
            SCCACHE_SERVER_UDS=str(root / "cache.sock"),
            SCCACHE_CONF=str(root / "roots.json"),
            SCCACHE_IDLE_TIMEOUT="0",
        )

        revision = _run(["git", "rev-parse", "HEAD"], ROOT, env)

        def add(tree: Path) -> None:
            _run(["git", "worktree", "add", "--detach", str(tree), revision], ROOT, env)
            stack.callback(_run, ["git", "worktree", "remove", "--force", str(tree)], ROOT, env)

        add(a)
        _run([str(wrapper), "--start-server"], a, env)
        stack.callback(
            subprocess.run,
            [str(wrapper), "--stop-server"],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        old_pid = build_cache.daemon_pid(env["SCCACHE_SERVER_UDS"])
        if old_pid is None:
            raise RuntimeError("private daemon failed to start")
        package, selected = FILTERS["app"]
        command = ["cargo", "test", "--locked", "-p", package, "--lib", selected, "--", "--exact"]
        observation = {
            "command": command,
            "revision": revision,
            "wrapper": str(wrapper),
            "original_daemon": old_pid,
        }
        with (output / "cargo.log").open("w") as log:
            build = subprocess.Popen(
                command,
                cwd=a,
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
            try:
                while build.poll() is None:
                    _require_disk(root, reserve)
                    if not b.exists() and (pid := _execution_compiler(build.pid)):
                        started = time.monotonic()
                        add(b)
                        exited = _wait_for_exit(old_pid)
                        observation.update(
                            compiler_pid=pid,
                            compiler="loopflow_execution",
                            registration_and_drain_seconds=round(time.monotonic() - started, 3),
                            compiler_outlived_daemon=exited
                            and _execution_compiler(build.pid) == pid,
                        )
                    try:
                        build.wait(timeout=1 if b.exists() else 0.2)
                    except subprocess.TimeoutExpired:
                        pass
            finally:
                if build.poll() is None:
                    os.killpg(build.pid, signal.SIGTERM)
                    build.wait()
        new_pid = build_cache.daemon_pid(env["SCCACHE_SERVER_UDS"])
        stats = json.loads(_run([str(wrapper), "--show-stats", "--stats-format", "json"], a, env))
        observation.update(
            cargo_exit=build.returncode,
            replacement_daemon=new_pid,
            new_root_loaded=b in map(Path, stats["basedirs"]),
            test_passed="test result: ok. 1 passed" in (output / "cargo.log").read_text(),
        )
        receipt.write_text(json.dumps(observation, indent=2) + "\n")
        print(receipt.read_text(), end="")
        if build.returncode or not observation["test_passed"]:
            raise RuntimeError(f"focused test failed; see {output / 'cargo.log'}")
        if not (
            observation.get("compiler_outlived_daemon")
            and new_pid
            and old_pid != new_pid
            and observation["new_root_loaded"]
        ):
            raise RuntimeError(
                "registration did not replace the daemon during the observed compilation"
            )


if __name__ == "__main__":
    main()

"""Verify automatic root inventory and idle-only reload with a private cache server."""

import argparse
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from build_cache import CACHE_ENV, VERBATIM_ENV_DEPS, refresh  # noqa: E402


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sccache", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    (output / "passed.json").unlink(missing_ok=True)
    receipts = []
    with tempfile.TemporaryDirectory(prefix="lf-roots-", dir="/tmp") as temporary:
        root = Path(temporary).resolve()
        a, b = root / "first", root / "later worktree"
        (a / "src").mkdir(parents=True)
        (a / "Cargo.toml").write_text(
            '[package]\nname="root-fixture"\nversion="0.1.0"\nedition="2021"\n'
            "[profile.dev]\nincremental=false\ndebug=1\n"
        )
        (a / "src/lib.rs").write_text("pub fn answer() -> u8 { 42 }\n")
        (a / "src/main.rs").write_text(
            'fn main() { println!("{}|{}", env!("CARGO_MANIFEST_DIR"), root_fixture::answer()); }\n'
        )
        (a / ".gitignore").write_text("/target\n")
        env = {
            key: value
            for key, value in os.environ.items()
            if not key.startswith(("LF_", "CARGO_", "SCCACHE_"))
            and key not in {"RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS"}
        }
        cache = str(args.sccache.resolve())
        env.update(
            RUSTC_WRAPPER=cache,
            SCCACHE_SERVER_UDS=str(root / "cache.sock"),
            SCCACHE_DIR=str(root / "cache"),
            SCCACHE_IDLE_TIMEOUT="30",
        )

        def run(argv: list[str], cwd: Path = a) -> str:
            result = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True)
            if result.returncode:
                raise RuntimeError(f"{argv!r}: {result.stdout}\n{result.stderr}")
            return result.stdout

        run(["git", "init", "-q"])
        run(["cargo", "generate-lockfile", "--offline"])
        run(["git", "add", "."])
        run(
            [
                "git",
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.com",
                "commit",
                "-qm",
                "initial",
            ]
        )
        config = root / "sccache.json"
        refresh(a, config)
        env.update(
            CACHE_ENV,
            SCCACHE_CONF=str(config),
            SCCACHE_BASEDIRS_VERBATIM_ENV_DEPS=VERBATIM_ENV_DEPS,
        )
        process = None

        def start() -> subprocess.Popen:
            # Retain our exact foreground server handle so idle exit is an
            # observed process completion, not a guessed sleep or a restart RPC.
            notify = root / "ready.sock"
            notify.unlink(missing_ok=True)
            with socket.socket(socket.AF_UNIX) as listener:
                listener.bind(str(notify))
                listener.listen(1)
                listener.settimeout(30)
                server_env = {
                    **env,
                    "SCCACHE_START_SERVER": "1",
                    "SCCACHE_NO_DAEMON": "1",
                    "SCCACHE_STARTUP_NOTIFY": str(notify),
                }
                log = (output / "server.log").open("a")
                child = subprocess.Popen([cache], env=server_env, stdout=log, stderr=log)
                log.close()
                try:
                    with listener.accept()[0] as ready:
                        ready.settimeout(30)
                        with ready.makefile("rb") as stream:
                            size = struct.unpack(">I", stream.read(4))[0]
                            if size == 0 or len(stream.read(size)) != size:
                                raise RuntimeError("cache server did not report startup")
                except BaseException:
                    child.terminate()
                    child.wait()
                    raise
            return child

        def stats() -> dict:
            return json.loads(run([cache, "--show-stats", "--stats-format", "json"]))["stats"]

        def build(label: str, tree: Path, target: Path, expected: tuple[int, int]) -> None:
            env["CARGO_TARGET_DIR"] = str(target)
            before = stats()
            raw = run(["cargo", "build", "--offline", "--jobs", "1", "--message-format=json"], tree)
            after = stats()
            hits = after["cache_hits"]["counts"].get("Rust", 0) - before["cache_hits"][
                "counts"
            ].get("Rust", 0)
            misses = after["cache_misses"]["counts"].get("Rust", 0) - before["cache_misses"][
                "counts"
            ].get("Rust", 0)
            identity = run([str(target / "debug/root-fixture")], tree).strip()
            receipt = {"case": label, "hits": hits, "misses": misses, "identity": identity}
            receipts.append(receipt)
            (output / f"{label}.jsonl").write_text(raw)
            (output / "receipts.json").write_text(json.dumps(receipts, indent=2) + "\n")
            if (hits, misses) != expected or identity != f"{tree}|42":
                raise RuntimeError(f"unexpected cache result: {receipt}")

        try:
            process = start()
            build("first", a, a / "target", (0, 1))
            run(["git", "worktree", "add", "--detach", str(b)])
            refresh(b, config)
            build("before_idle", b, b / "target", (0, 1))
            # Observe natural expiry independently of the registration hook.
            if process.wait(timeout=60) != 0:
                raise RuntimeError("cache server failed instead of expiring idle")
            (b / "target").rename(root / "retained-before-idle-target")
            process = start()
            build("after_idle", b, b / "target", (1, 0))
            (output / "configuration.json").write_text(config.read_text())
            (output / "passed.json").write_text('{"passed": true}\n')
        finally:
            if process is not None and process.poll() is None:
                process.terminate()
                process.wait()


if __name__ == "__main__":
    main()

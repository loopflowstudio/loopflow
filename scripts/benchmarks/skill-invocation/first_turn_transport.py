"""Compare file-backed first turns on native Codex exec and terminal surfaces."""

import argparse
import fcntl
import json
import os
import pty
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import termios
import threading
import time
from pathlib import Path

from context_delivery import Requests


def _assess(requests: list[dict], prompt: str) -> dict[str, bool]:
    texts = [
        block.get("text", "")
        for request in requests
        for item in request.get("input", [])
        if item.get("role") == "user"
        for block in item.get("content", [])
        if isinstance(block, dict)
    ]
    return {
        "one_model_request": len(requests) == 1,
        "complete_first_turn": bool(texts) and texts[-1] == prompt and texts.count(prompt) == 1,
    }


def _run(command: list[str], root: Path, env: dict[str, str]) -> tuple[int, bytes, bool]:
    master, slave = pty.openpty()

    def terminal() -> None:
        os.setsid()
        # stdout is a real controlling terminal; stdin alone is the prompt file.
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

    try:
        with (root / "prompt.txt").open("rb") as source:
            process = subprocess.Popen(
                command,
                cwd=root / "work",
                env=env,
                stdin=source,
                stdout=slave,
                stderr=slave,
                preexec_fn=terminal,
            )
    except BaseException:
        os.close(master)
        raise
    finally:
        os.close(slave)
    output = bytearray()
    deadline = time.monotonic() + 30
    timed_out = False
    try:
        while True:
            if time.monotonic() >= deadline:
                timed_out = True
                break
            if not select.select([master], [], [], 0.1)[0]:
                if process.poll() is not None:
                    break
                continue
            try:
                data = os.read(master, 65536)
            except OSError:
                break
            if not data:
                break
            output.extend(data)
            if b"\x1b[6n" in data:
                os.write(master, b"\x1b[1;1R")
            if b"\x1b[c" in data:
                os.write(master, b"\x1b[?1;2c")
        try:
            process.wait(timeout=max(0.1, deadline - time.monotonic()))
        except subprocess.TimeoutExpired:
            timed_out = True
    finally:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)
        os.close(master)
    return process.returncode, bytes(output), timed_out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    executable = shutil.which("codex")
    if executable is None:
        raise SystemExit("Codex is required")
    executable = str(Path(executable).resolve())
    args.output.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="stdin-", dir=args.output)).resolve()
    for name in ["home", "native", "work"]:
        (root / name).mkdir()
    prompt = "LOO444_BEGIN\n" + "all first-turn bytes survive.\n" * 7500 + "LOO444_END"
    (root / "prompt.txt").write_text(prompt)
    env = {
        "PATH": os.environ["PATH"],
        "HOME": str(root / "home"),
        "CODEX_HOME": str(root / "native"),
        "TERM": "xterm-256color",
    }
    version = subprocess.run(
        [executable, "--version"],
        cwd=root / "work",
        env=env,
        capture_output=True,
        text=True,
        timeout=10,
        check=True,
    ).stdout.strip()
    server = Requests("codex")
    serving = threading.Thread(target=server.serve_forever, daemon=True)
    serving.start()
    (root / "native/config.toml").write_text(f'''model = "gpt-5.4"
model_provider = "fixture"
cli_auth_credentials_store = "file"
allow_login_shell = false
sandbox_mode = "danger-full-access"
approval_policy = "never"
[features]
shell_snapshot = false
[model_providers.fixture]
name = "Local fixture"
base_url = "http://127.0.0.1:{server.server_port}/v1"
wire_api = "responses"
requires_openai_auth = false
[analytics]
enabled = false
[feedback]
enabled = false
[projects."{root / "work"}"]
trust_level = "trusted"
''')
    observations = {}
    try:
        for surface, options in [
            ("headless", ["exec", "--skip-git-repo-check", "--json", "-"]),
            ("terminal", ["--no-alt-screen", "--no-daemon", "-"]),
        ]:
            command = [executable, *options]
            status, output, timed_out = _run(command, root, env)
            bodies = list(server.bodies)
            (root / f"{surface}.output").write_bytes(output)
            (root / f"{surface}.requests.json").write_text(json.dumps(bodies, indent=2))
            observations[surface] = {
                "exit": status,
                "timed_out": timed_out,
                "max_argument_bytes": max(len(arg.encode()) for arg in command),
                "checks": _assess(bodies, prompt),
            }
            server.bodies.clear()
        result = {
            "version": version,
            "evidence": str(root),
            "prompt_bytes": len(prompt.encode()),
            **observations,
        }
        (root / "result.json").write_text(json.dumps(result, indent=2))
        print(json.dumps(result, indent=2))
        return (
            0
            if all(
                result["exit"] == 0 and not result["timed_out"] and all(result["checks"].values())
                for result in observations.values()
            )
            else 1
        )
    finally:
        server.shutdown()
        server.server_close()
        serving.join(timeout=5)


if __name__ == "__main__":
    sys.exit(main())

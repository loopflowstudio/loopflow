"""Reproduce startup-hook/input fencing with Claude and a disposable home.

Run with `uv run python scratch/claude-startup-fence.py`.
No complete input is sent while the hook holds: the model endpoint is loopback.
"""

import fcntl
import json
import os
from pathlib import Path
import select
import shlex
import shutil
import subprocess
import sys
import tempfile
import time


def main() -> None:
    executable = shutil.which("claude")
    if executable is None:
        raise RuntimeError("Claude is unavailable; no configured probe ran")
    version = subprocess.check_output([executable, "--version"], text=True).strip()
    with tempfile.TemporaryDirectory(prefix="lf-steer-fence-") as temp:
        root = Path(temp)
        marker = root / "startup"
        lock = root / "session.lock"
        hook = root / "hook.py"
        settings = root / "settings.json"
        hook.write_text(
            "import fcntl\nfrom pathlib import Path\n"
            f"Path({str(marker)!r}).touch()\n"
            f"with open({str(lock)!r}, 'w') as lock:\n"
            " fcntl.flock(lock, fcntl.LOCK_EX)\n print('{}')\n"
        )
        settings.write_text(json.dumps({"hooks": {"SessionStart": [{"hooks": [{
            "type": "command", "command": f"{shlex.quote(sys.executable)} {shlex.quote(str(hook))}", "timeout": 10,
        }]}]}}))
        environment = {
            "PATH": "/usr/bin:/bin", "HOME": temp,
            "CLAUDE_CONFIG_DIR": str(root / "claude"),
            "ANTHROPIC_BASE_URL": "http://127.0.0.1:9",
            "ANTHROPIC_AUTH_TOKEN": "fixture-only",
            "DISABLE_AUTOUPDATER": "1", "DISABLE_TELEMETRY": "1",
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1",
        }
        with lock.open("w") as fence, (root / "out").open("w") as out, (root / "err").open("w") as err:
            fcntl.flock(fence, fcntl.LOCK_EX)
            process = subprocess.Popen(
                [executable, "--print", "--input-format", "stream-json",
                 "--output-format", "stream-json", "--verbose", "--settings", str(settings)],
                env=environment, cwd=temp, stdin=subprocess.PIPE, stdout=out, stderr=err,
                start_new_session=True,
            )
            try:
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline and not marker.exists() and process.poll() is None:
                    time.sleep(0.02)
                if not marker.exists():
                    raise RuntimeError("Startup hook did not run; no fencing conclusion")
                payload = (json.dumps({"type": "user", "message": {
                    "role": "user", "content": "x" * 131_072,
                }}) + "\n").encode()
                if process.stdin is None:
                    raise RuntimeError("Provider stdin was not piped")
                os.set_blocking(process.stdin.fileno(), False)
                sent = 0
                started = time.monotonic()
                while sent < len(payload) and time.monotonic() - started < 2:
                    try:
                        sent += os.write(process.stdin.fileno(), payload[sent:])
                    except BlockingIOError:
                        select.select([], [process.stdin], [], 0.02)
                print(json.dumps({
                    "provider": version, "startup_hook_before_stdin": True,
                    "input_bytes": len(payload), "sent_bytes": sent,
                    "completed_within_two_second_dispatch_deadline": sent == len(payload),
                }))
            finally:
                # Stop the owned process group before releasing the hook, so no
                # complete request proceeds to even the loopback model endpoint.
                if process.poll() is None:
                    os.killpg(process.pid, 15)
                    try:
                        process.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, 9)
                        process.wait()
                fcntl.flock(fence, fcntl.LOCK_UN)


if __name__ == "__main__":
    main()

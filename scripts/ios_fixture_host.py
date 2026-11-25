"""Run Machine commands for the iOS tests, which cannot run them in the Simulator.

A process the Simulator starts cannot look up the OS account, and `lf` finds its
installation from that account. `scripts/test_ios.py` starts this host beside
`xcodebuild` and names its socket in `LOOPFLOW_FIXTURE_HOST`. Each connection
carries one JSON request line and gets one JSON answer line:

    {"argv": [...], "env": {...}, "cwd": "..."}     -> {"status", "stdout", "stderr"}
    {... "background": true}                         -> {"pid"}
    {"stop": pid}                                    -> {"status"}

A command runs with exactly the environment it names. Background commands this
host started and that are still running when it exits are stopped.
"""

import json
import os
import signal
import socketserver
import subprocess
import sys
import threading

_started: dict[int, subprocess.Popen] = {}
_lock = threading.Lock()


def _run(request: dict) -> dict:
    if "stop" in request:
        with _lock:
            child = _started.pop(int(request["stop"]), None)
        if child is None:
            return {"status": None}
        child.terminate()
        return {"status": child.wait(timeout=30)}
    options = {"env": request["env"], "cwd": request["cwd"], "stdin": subprocess.DEVNULL}
    if request.get("background"):
        child = subprocess.Popen(
            request["argv"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
            **options,
        )
        with _lock:
            _started[child.pid] = child
        return {"pid": child.pid}
    done = subprocess.run(request["argv"], capture_output=True, text=True, timeout=300, **options)
    return {"status": done.returncode, "stdout": done.stdout, "stderr": done.stderr}


class _Handler(socketserver.StreamRequestHandler):
    def handle(self) -> None:
        try:
            answer = _run(json.loads(self.rfile.readline()))
        except Exception as error:  # The test reads the failure as its answer.
            answer = {"status": 127, "stdout": "", "stderr": f"fixture host: {error}"}
        self.wfile.write((json.dumps(answer) + "\n").encode())


class _Server(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True


def _stop_started() -> None:
    with _lock:
        children = list(_started.values())
        _started.clear()
    for child in children:
        child.terminate()
    for child in children:
        try:
            child.wait(timeout=30)
        except subprocess.TimeoutExpired:
            child.kill()


def _exit(_signum: int, _frame: object) -> None:
    raise SystemExit(0)


def main() -> int:
    path = sys.argv[1]
    signal.signal(signal.SIGTERM, _exit)
    server = _Server(path, _Handler)
    os.chmod(path, 0o600)
    try:
        server.serve_forever()
    finally:
        server.server_close()
        _stop_started()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

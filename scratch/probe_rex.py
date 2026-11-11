"""Probe an isolated Rex server; never launch a Rex app or a real provider."""

import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time


def _child(root: Path, name: str) -> None:
    receipt = {"pid": os.getpid(), "cwd": os.getcwd(), "at_ns": time.time_ns()}
    receipt["ps"] = subprocess.check_output(
        ["/bin/ps", "-p", str(os.getpid()), "-o", "pid=,ppid=,pgid=,lstart=,comm="],
        text=True,
    ).strip()
    with (root / f"{name}.jsonl").open("a") as output:
        output.write(json.dumps(receipt) + "\n")
    print(f"\033]7;file://localhost{root}\007READY {name}", flush=True)
    # The hard limit also bounds orphan lifetime if the probe runner is killed.
    if select.select([sys.stdin], [], [], 20)[0]:
        command = sys.stdin.readline().strip()
        if command == "crash":
            os.kill(os.getpid(), signal.SIGKILL)
        sys.exit(int(command or "0"))
    sys.exit(75)


def main() -> None:
    rex = shutil.which("rex")
    if rex is None:
        raise RuntimeError("Rex executable is required")
    rex = str(Path(rex).resolve())
    report = {"version": subprocess.check_output([rex, "version"], text=True),
              "sha256": hashlib.sha256(Path(rex).read_bytes()).hexdigest(),
              "observations": {}}
    observations = report["observations"]
    with tempfile.TemporaryDirectory(prefix="loo820-", dir="/tmp") as temporary:
        root = Path(temporary)
        endpoint = f"unix://{root}/rex.sock"
        config = root / "empty.lua"
        config.write_text("")
        environment = {"HOME": str(root), "XDG_CONFIG_HOME": str(root / "config"),
                       "XDG_STATE_HOME": str(root / "xdg-state"),
                       "PATH": "/usr/bin:/bin", "TERM": "xterm-256color"}
        server = None
        sessions = []
        identities = {}

        def call(*args: str) -> dict:
            result = subprocess.run(
                [rex, "--autostart=false", "--timeout", "2s", "-S", endpoint, *args],
                env=environment, stdin=subprocess.DEVNULL, capture_output=True,
                text=True, timeout=8,
            )
            try:
                body = json.loads(result.stdout)
            except ValueError:
                body = result.stdout
            return {"code": result.returncode, "body": body, "stderr": result.stderr}

        def start() -> subprocess.Popen:
            (root / "rex.sock").unlink(missing_ok=True)
            process = subprocess.Popen(
                [rex, "server", "run", "--listen", endpoint, "--state-dir",
                 str(root / "state"), "--config", str(config)],
                env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL, start_new_session=True,
            )
            for _ in range(100):
                if (root / "rex.sock").exists():
                    return process
                if process.poll() is not None:
                    raise RuntimeError("isolated Rex exited before listening")
                time.sleep(0.03)
            process.terminate()
            process.wait(timeout=5)
            raise RuntimeError("isolated Rex did not listen")

        def stop(process: subprocess.Popen) -> None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)

        def create(name: str) -> tuple[str, str]:
            created = call("new", name, "--json", "--keep-open", "--shell=none",
                           "--cwd", str(root), "--", sys.executable,
                           str(Path(__file__).resolve()), "--child", str(root), name)
            observations[f"{name}_created"] = created
            if created["code"]:
                raise RuntimeError(created)
            body = created["body"]
            session = body["session_id"]
            sessions.append(session)
            blocks = call("api", "call", "session.list_blocks",
                          json.dumps({"session_id": session}))
            block = blocks["body"]["blocks"][0]["block_id"]
            for _ in range(100):
                if (root / f"{name}.jsonl").exists():
                    break
                time.sleep(0.02)
            return session, block

        def block_call(session: str, block: str, method: str, *args: str) -> dict:
            result = call("block", "call", "-s", session, "-b", block, method, *args)
            if method == "process" and result["code"] == 0:
                child = result["body"].get("child")
                if child:
                    pid = child["pid"]
                    identities[pid] = subprocess.run(
                        ["/bin/ps", "-p", str(pid), "-o", "lstart=,comm="],
                        capture_output=True, text=True,
                    ).stdout.strip()
            return result

        try:
            server = start()
            methods = call("api", "list", "--json")
            observations["method_names"] = [
                method["name"] for method in methods["body"]["methods"]
                if "terminal" in method["name"] or "event" in method["name"]
            ]
            observations["process_schema"] = call(
                "api", "describe", "com.superlogical.terminal.process")
            blocks_by_name = {}
            for name, command in [("normal", "42"), ("crash", "crash")]:
                session, block = create(name)
                blocks_by_name[name] = (session, block)
                observations[f"{name}_process"] = block_call(session, block, "process")
                observations[f"{name}_pwd"] = block_call(session, block, "pwd")
                observations[f"{name}_write"] = block_call(
                    session, block, "write", json.dumps({"data": command + "\n"}))
                time.sleep(0.25)
                # Fresh CLI connections, both opened after exit.
                observations[f"{name}_wait"] = call("wait", block, "--json", "--for", "2s")
                observations[f"{name}_wait_again"] = call("wait", block, "--json", "--for", "2s")
                observations[f"{name}_exited_process"] = block_call(session, block, "process")
            session, block = create("restart")
            observations["restart_before"] = block_call(session, block, "process")
            time.sleep(2)  # Rex saves newly created sessions asynchronously.
            stop(server)
            server = None
            server = start()
            time.sleep(0.5)
            observations["restart_after"] = block_call(session, block, "process")
            observations["normal_process_after_restart"] = block_call(
                *blocks_by_name["normal"], "process")
            observations["crash_process_after_restart"] = block_call(
                *blocks_by_name["crash"], "process")
            observations["normal_wait_after_restart"] = call(
                "wait", blocks_by_name["normal"][1],
                "--json", "--for", "2s")
            observations["restart_write"] = block_call(
                session, block, "write", json.dumps({"data": "exit 0\n"}))
            observations["restart_wait"] = call("wait", block, "--json", "--for", "2s")
        finally:
            if server is not None:
                for session in sessions:
                    call("kill", session)
                stop(server)
            report["launches"] = {
                file.stem: [json.loads(line) for line in file.read_text().splitlines()]
                for file in root.glob("*.jsonl")
            }
            # Bounded children above receive HUP on PTY closure; retain any survivor
            # as a cleanup failure rather than claiming successful teardown.
            time.sleep(0.1)
            report["survivors"] = []
            for pid, identity in identities.items():
                current = subprocess.run(
                    ["/bin/ps", "-p", str(pid), "-o", "lstart=,comm="],
                    capture_output=True, text=True,
                ).stdout.strip()
                if identity and current == identity:
                    report["survivors"].append({"pid": pid, "identity": identity})
            destination = Path(__file__).with_name("rex-probe-receipts.json")
            destination.write_text(json.dumps(report, indent=2) + "\n")
            print(destination)
    assert observations["normal_wait"]["body"]["exit_code"] == 42
    assert observations["normal_wait_again"]["body"]["exit_code"] == 42
    assert observations["crash_wait"]["body"]["exit_code"] == -1
    assert observations["crash_wait_again"]["body"]["exit_code"] == -1
    assert report["survivors"] == [], report["survivors"]


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--child":
        _child(Path(sys.argv[2]), sys.argv[3])
    else:
        main()

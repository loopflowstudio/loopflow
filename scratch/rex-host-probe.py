"""Probe public Rex APIs in an isolated server; never opens an app or provider."""

import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import tempfile
import time


def _run() -> None:
    env = {k: v for k, v in os.environ.items() if not k.startswith(("LF_", "REX_"))}
    with tempfile.TemporaryDirectory(prefix="loo498-", dir="/tmp") as directory:
        root = Path(directory)
        env["HOME"] = directory
        config = root / "empty.lua"
        config.write_text("")
        endpoint = f"unix://{root}/rex.sock"
        cli = ["rex", "--autostart=false", "--timeout", "2s", "-S", endpoint]

        def call(method: str, **arguments: object) -> dict:
            output = subprocess.check_output(
                [*cli, "api", "call", method, json.dumps(arguments)], env=env,
                stderr=subprocess.PIPE, timeout=5, text=True,
            )
            return json.loads(output)

        with (root / "server.log").open("w") as log:
            server = subprocess.Popen(
                ["rex", "server", "run", "--listen", endpoint, "--state-dir",
                 str(root / "state"), "--config", str(config)],
                env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                start_new_session=True,
            )
            try:
                for _ in range(50):
                    if (root / "rex.sock").exists():
                        break
                    if server.poll() is not None:
                        raise RuntimeError("Isolated Rex server exited")
                    time.sleep(0.1)
                block = {"block": {"flavor": "com.superlogical.terminal.shell",
                    "options": {"shell": "none", "command": ["/bin/sh", "-c",
                    "printf 'probe-start\\n'; exec /bin/cat"], "cwd": str(root)}}}
                created = call("session.create", label="LOO-498 probe", initial_windows=[{
                    "window_label": "Task", "layout": {"split": {
                        "direction": "horizontal", "ratio": 0.6,
                        "before": block, "after": block}}}])
                sid = created["session_id"]
                first, second = created["initial_windows"][0]["block_ids"]
                before = call("session.view", session_id=sid)
                call("session.detach_block", session_id=sid, block_id=second)
                detached = call("session.list_blocks", session_id=sid)
                call("session.move_block", session_id=sid, block_id=second,
                     anchor_block_id=first, direction="horizontal", ratio=0.6)
                restored = call("session.view", session_id=sid)
                assert before["active_window_id"] == restored["active_window_id"]
                assert before["windows"][0]["focused_block_id"] == restored["windows"][0]["focused_block_id"]
                assert second in json.dumps(detached) and "detached" in json.dumps(detached)
                subprocess.run([*cli, "send", "-b", second, "retained-input\n"],
                               env=env, check=True, capture_output=True, timeout=5)
                screen = ""
                for _ in range(30):
                    screen = subprocess.check_output([*cli, "capture", "-b", second],
                                                     env=env, text=True, timeout=5)
                    if "retained-input" in screen:
                        break
                    time.sleep(0.1)
                assert screen.count("probe-start") == 1 and "retained-input" in screen
                try:
                    call("session.new_window", session_id=sid, focus=False,
                         layout={"block": {"flavor": "nonexistent.probe.block"}})
                except subprocess.CalledProcessError:
                    pass
                else:
                    raise AssertionError("Invalid flavor unexpectedly succeeded")
                assert call("session.view", session_id=sid) == restored
                call("session.detach_block", session_id=sid, block_id=second)
                call("session.detach_block", session_id=sid, block_id=first)
                empty = call("session.view", session_id=sid)
                assert not empty.get("windows")
                assert first in json.dumps(call("session.list_blocks", session_id=sid))
                try:
                    call("session.move_block", session_id=sid, block_id=second,
                         anchor_block_id=first)
                except subprocess.CalledProcessError:
                    print("CONFIRMED: all-detached session has no anchor for move_block")
                else:
                    raise AssertionError("Detached anchor unexpectedly accepted")
                call("session.destroy", session_id=sid)
                assert sid not in json.dumps(call("session.list"))
                # Per-block cwd is not evidence of the host's native split default.
                windows = []
                for name in ("task-a", "task-b"):
                    cwd = root / name
                    cwd.mkdir()
                    windows.append({"window_label": name, "layout": {"block": {
                        "flavor": "com.superlogical.terminal.shell", "options": {
                            "cwd": str(cwd), "shell": "none", "command": [
                                "/bin/sh", "-c",
                                'printf "REX_SERVER=%s\\n" "${REX_SERVER-unset}"; exec /bin/cat',
                            ],
                        },
                    }}})
                split_probe = call("session.create", label="Native split defaults",
                                   initial_windows=windows)
                sid = split_probe["session_id"]
                for name, window in zip(("task-a", "task-b"), split_probe["initial_windows"]):
                    source = window["block_ids"][0]
                    created_split = call("session.new_split", session_id=sid,
                        anchor_id=source, direction="horizontal", side="after", focus=False,
                        layout={"block": {"flavor": "com.superlogical.terminal.shell"}})
                    native = created_split["block_ids"][0]
                    marker = root / f"{name}.pwd"
                    subprocess.run([*cli, "send", "-b", native,
                        f"pwd -P > {shlex.quote(str(marker))}\n"],
                        env=env, check=True, capture_output=True, timeout=5)
                    for _ in range(30):
                        if marker.exists() and marker.read_text().strip():
                            break
                        time.sleep(0.1)
                    actual = marker.read_text().strip()
                    print("NATIVE SPLIT:", json.dumps({
                        "source": name,
                        "task_cwd_inherited": Path(actual).resolve() == (root / name).resolve(),
                        "home_fallback": Path(actual).resolve() == root.resolve(),
                    }))
                    print("CALLER:", subprocess.check_output(
                        [*cli, "capture", "-b", source], env=env, text=True, timeout=5).strip())
                call("session.destroy", session_id=sid)
                marker = root / "starts"
                restart_block = {"block": {"flavor": "com.superlogical.terminal.shell",
                    "options": {"shell": "none", "command": ["/bin/sh", "-c",
                    'echo start >> "$1"; exec /bin/cat', "probe", str(marker)],
                    "cwd": str(root)}}}
                saved = call("session.create", label="Restart probe", initial_windows=[{
                    "layout": restart_block}])
                for _ in range(30):
                    if marker.exists():
                        break
                    time.sleep(0.1)
                assert marker.read_text().splitlines() == ["start"]
                time.sleep(2)
                server.terminate()
                server.wait(timeout=5)
                server = subprocess.Popen(
                    ["rex", "server", "run", "--listen", endpoint, "--state-dir",
                     str(root / "state"), "--config", str(config)],
                    env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                    start_new_session=True,
                )
                for _ in range(50):
                    try:
                        sessions = call("session.list")
                        break
                    except (subprocess.CalledProcessError, FileNotFoundError):
                        time.sleep(0.1)
                time.sleep(0.3)
                restarted_view = call("session.view", session_id=saved["session_id"])
                process = call("com.superlogical.terminal.process", session_id=saved["session_id"],
                               block_id=saved["initial_windows"][0]["block_ids"][0], args={})
                assert saved["initial_windows"][0]["block_ids"][0] in json.dumps(restarted_view)
                assert len(marker.read_text().splitlines()) == 1
                print("RESTART:", json.dumps({
                    "session_id_retained": saved["session_id"] in json.dumps(sessions),
                    "command_starts": len(marker.read_text().splitlines()),
                    "foreground_program": process.get("foreground", {}).get("name"),
                }))
                for session in sessions.get("sessions", []):
                    call("session.destroy", session_id=session["session_id"])
                print("PASS: isolated create/split, detached identity/input retention, focus, failed-update preservation, explicit cleanup")
            finally:
                os.killpg(server.pid, signal.SIGTERM)
                try:
                    server.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=5)


if __name__ == "__main__":
    _run()

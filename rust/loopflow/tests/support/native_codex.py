"""Codex protocol and terminal stand-in: require the terminal to answer approvals."""

from __future__ import annotations

import base64
import hashlib
import json
import os
import socket
import struct
import sys
import threading
import time
import uuid
from pathlib import Path

from codex_socket import _message, _read


def _send(stream: socket.socket, value: dict, client: bool = False) -> None:
    data = json.dumps(value).encode()
    size = len(data)
    mask = os.urandom(4) if client else b""
    flag = 128 if client else 0
    if size < 126:
        header = bytes([0x81, flag | size])
    elif size < 65536:
        header = bytes([0x81, flag | 126]) + struct.pack("!H", size)
    else:
        header = bytes([0x81, flag | 127]) + struct.pack("!Q", size)
    if mask:
        data = bytes(byte ^ mask[index % 4] for index, byte in enumerate(data))
    stream.sendall(header + mask + data)


def _header(stream: socket.socket) -> bytes:
    header = bytearray()
    while not header.endswith(b"\r\n\r\n"):
        header.extend(_read(stream, 1))
    return bytes(header)


def _client(endpoint: str, home: Path, thread: str | None) -> None:
    if (home / "close-terminal").exists():
        return
    if (home / "fail-terminal").exists():
        raise SystemExit(24)
    if native_home := os.environ.get("CODEX_HOME"):
        Path(native_home).mkdir(parents=True, exist_ok=True)
        (Path(native_home) / "opened").write_text(os.environ.get("LF_CAPTURE_KEY", ""))
    print("\n".join(sys.argv[1:]), flush=True)
    with (home / "launched").open("a") as file:
        file.write(os.environ.get("LF_CAPTURE_KEY", "") + "\n")
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(15)
        stream.connect(endpoint)
        stream.sendall(
            b"GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\n"
            b"Connection: Upgrade\r\nSec-WebSocket-Version: 13\r\n"
            b"Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n"
        )
        _header(stream)
        _send(
            stream,
            {
                "id": 1,
                "method": "initialize",
                "params": {"clientInfo": {"name": "native-fixture", "version": "1"}},
            },
            True,
        )
        assert json.loads(_message(stream))["id"] == 1
        _send(stream, {"method": "initialized"}, True)
        if started := os.environ.get("FIXTURE_STARTED"):
            Path(started).touch()
            deadline = time.monotonic() + 75
            while not Path(os.environ["FIXTURE_STOP"]).exists():
                if time.monotonic() >= deadline:
                    raise TimeoutError("idle terminal was not released")
                time.sleep(0.1)
            return
        _send(
            stream,
            {
                "id": 2,
                "method": "thread/resume" if thread else "thread/start",
                "params": {
                    **({"threadId": thread} if thread else {}),
                    "approvalPolicy": "on-request",
                    "threadSource": "user",
                    "config": {"native-setting": True},
                },
            },
            True,
        )
        while True:
            rpc = json.loads(_message(stream))
            if rpc.get("id") == 2:
                assert "error" not in rpc, rpc
                thread = rpc["result"]["thread"]["id"]
            if rpc.get("id") == 2 and rpc["result"]["thread"]["turns"]:
                _send(
                    stream,
                    {
                        "id": 3,
                        "method": "turn/start",
                        "params": {
                            "threadId": thread,
                            "input": [{"type": "text", "text": "Native follow-up"}],
                        },
                    },
                    True,
                )
            if rpc.get("method") == "turn/started" and (home / "fail-after-submit").exists():
                raise SystemExit(25)
            if rpc.get("method") == "item/commandExecution/requestApproval":
                if (home / "wait-approval").exists():
                    (home / "pending-request.json").write_text(json.dumps(rpc))
                    print("permission pending", flush=True)
                    with open("/dev/tty") as terminal:
                        assert terminal.readline().strip() == "decline"
                _send(stream, {"id": rpc["id"], "result": {"decision": "decline"}}, True)
            if rpc.get("method") == "turn/completed":
                if "TITLE_HOST" in os.environ:
                    (home / "args").write_text("\n".join(sys.argv[1:]))
                    (home / "ready").touch()
                    deadline = time.monotonic() + 30
                    while not (home / "release").exists() and time.monotonic() < deadline:
                        time.sleep(.02)
                    print("latest agent message: done", flush=True)
                stream.sendall(b"\x88\x80\x00\x00\x00\x00")
                try:
                    _message(stream)
                except EOFError:
                    pass
                return


def _server(endpoint: str, home: Path) -> None:
    turns_file = home / "native-turns.json"
    turns = json.loads(turns_file.read_text()) if turns_file.exists() else []
    thread_file = home / "native-thread"
    thread = thread_file.read_text() if thread_file.exists() else ""
    lock = threading.Lock()

    def serve(stream: socket.socket) -> None:
        nonlocal thread, turns
        with stream:
            header = _header(stream)
            key = next(
                line.split(b":", 1)[1].strip()
                for line in header.split(b"\r\n")
                if line.lower().startswith(b"sec-websocket-key:")
            )
            accept = base64.b64encode(
                hashlib.sha1(key + b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11").digest()
            )
            stream.sendall(
                b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n"
                b"Connection: Upgrade\r\nSec-WebSocket-Accept: " + accept + b"\r\n\r\n"
            )
            try:
                while True:
                    rpc = json.loads(_message(stream))
                    method = rpc.get("method")
                    params = rpc.get("params", {})
                    if method is None:
                        with lock:
                            (home / "native-approval.json").write_text(json.dumps(rpc["result"]))
                            (home / "native-answer.json").write_text(json.dumps(rpc))
                            turn = turns[-1]
                            turn["status"] = "completed"
                            turns_file.write_text(json.dumps(turns))
                        _send(
                            stream,
                            {
                                "method": "turn/completed",
                                "params": {"threadId": thread, "turn": turn},
                            },
                        )
                        continue
                    if "id" not in rpc:
                        continue
                    result = {}
                    if method in ("thread/start", "thread/resume"):
                        if method == "thread/start":
                            thread = "thread-" + uuid.uuid4().hex
                            turns = []
                            thread_file.write_text(thread)
                        elif not turns or params.get("threadId") != thread:
                            _send(
                                stream,
                                {
                                    "id": rpc["id"],
                                    "error": {"code": -32600, "message": "no rollout found"},
                                },
                            )
                            continue
                        result = {"thread": {"id": thread, "turns": turns}}
                        if "developerInstructions" in params:
                            (home / "native-config.json").write_text(json.dumps(params))
                    elif method == "thread/turns/list":
                        result = {"data": turns, "nextCursor": None}
                    elif method == "thread/loaded/list":
                        result = {"data": [thread] if thread else [], "nextCursor": None}
                    elif method == "config/read":
                        result = {
                            "layers": [{"name": {"type": "sessionFlags"}, "config": {
                                "hooks": {"SessionStart": [{"matcher": "startup", "hooks": [
                                    {"type": "command", "command": "host-hook"}
                                ]}], "state": {"host-trust": {"trusted_hash": "unchanged"}}}
                            }}],
                            "config": {
                                "sandbox_workspace_write": {
                                    "writable_roots": [str(home / "configured-root")]
                                }
                            }
                        }
                    elif method == "turn/start":
                        with lock:
                            turn = {
                                "id": f"turn-{len(turns)}",
                                "status": "inProgress",
                                "items": [
                                    {
                                        "id": f"user-{len(turns)}",
                                        "type": "userMessage",
                                        "content": params["input"],
                                    }
                                ],
                            }
                            turns.append(turn)
                            turns_file.write_text(json.dumps(turns))
                        result = {"turn": turn}
                    _send(stream, {"id": rpc["id"], "result": result})
                    if method == "turn/start":
                        _send(
                            stream,
                            {
                                "method": "turn/started",
                                "params": {"threadId": thread, "turn": turn},
                            },
                        )
                        _send(
                            stream,
                            {
                                "id": "approval",
                                "method": "item/commandExecution/requestApproval",
                                "params": {"threadId": thread, "turnId": turn["id"]},
                            },
                        )
            except (EOFError, OSError):
                pass

    with socket.socket(socket.AF_UNIX) as listener:
        listener.bind(endpoint)
        listener.listen()
        while True:
            client, _ = listener.accept()
            threading.Thread(target=serve, args=(client,), daemon=True).start()


def main() -> None:
    args = sys.argv[1:]
    home = Path(os.environ["LF_HOME"])
    # A wrapper can reject long argv without ever seeing the large payload.
    assert all(len(arg.encode()) < 1024 for arg in args), args
    assert "CODEX_ACCESS_TOKEN" not in os.environ
    with (home / "native-argv.jsonl").open("a") as file:
        file.write(json.dumps(args) + "\n")
    if "--remote" in args:
        assert not {"--no-daemon", "--add-dir", "--sandbox", "--ask-for-approval"}.intersection(
            args
        )
        _client(
            args[args.index("--remote") + 1].removeprefix("unix://"),
            home,
            args[-1] if "resume" in args else None,
        )
    elif "--listen" in args:
        if (home / "fail-server").exists():
            raise SystemExit(23)
        if (home / "stall-server").exists():
            threading.Event().wait(60)
        _server(args[args.index("--listen") + 1].removeprefix("unix://"), home)


if __name__ == "__main__":
    main()

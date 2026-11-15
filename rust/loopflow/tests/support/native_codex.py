"""Codex protocol and terminal stand-in: require the terminal to answer approvals."""

from __future__ import annotations

import base64
import hashlib
import json
import os
import select
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


def _client(endpoint: str, home: Path, thread: str) -> None:
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
                "method": "thread/resume",
                "params": {"threadId": thread, "approvalPolicy": "on-request"},
            },
            True,
        )
        while True:
            rpc = json.loads(_message(stream))
            if rpc.get("id") == 2:
                assert "error" not in rpc, rpc
                assert rpc["result"]["thread"]["id"] == thread
            turns = rpc["result"]["thread"]["turns"] if rpc.get("id") == 2 else []
            if turns and all(turn["status"] != "inProgress" for turn in turns):
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
            # The first turn may already be running when this client joins.
            running = rpc.get("method") == "turn/started" or any(
                turn["status"] == "inProgress" for turn in turns
            )
            if running and (home / "fail-after-submit").exists():
                raise SystemExit(25)
            if running and (home / "idle-terminal").exists():
                # Like Codex, the one idle terminal ignores losing its server until signalled.
                (home / "idle-terminal").unlink()
                (home / "idling").write_text(str(os.getpid()))
                time.sleep(75)
                raise TimeoutError("idle terminal was not signalled")
            if rpc.get("method") == "item/commandExecution/requestApproval":
                if (home / "wait-approval").exists():
                    (home / "pending-request.json").write_text(json.dumps(rpc))
                    print("permission pending", flush=True)
                    with open("/dev/tty") as terminal:
                        assert terminal.readline().strip() == "decline"
                _send(stream, {"id": rpc["id"], "result": {"decision": "decline"}}, True)
            if rpc.get("method") == "turn/completed":
                if (home / "reattach-service-tier").exists():
                    initial = _request(stream, "thread/resume", {"threadId": thread})
                    _request(stream, "thread/settings/update", {
                        "threadId": thread, "serviceTier": "priority",
                    })
                    (home / "native-tier-before.json").write_text(json.dumps(initial))
                    with open("/dev/tty") as terminal:
                        deadline = time.monotonic() + 30
                        while not select.select([terminal], [], [], .1)[0]:
                            if (home / "release").exists():
                                return
                            assert time.monotonic() < deadline, "no reattached input"
                        assert terminal.readline().strip() == "inspect tier"
                    retained = _request(stream, "thread/resume", {"threadId": thread})
                    (home / "native-tier-after.json").write_text(json.dumps(retained))
                    deadline = time.monotonic() + 30
                    while not (home / "release").exists():
                        assert time.monotonic() < deadline, "reattached terminal not released"
                        time.sleep(.02)
                if (home / "check-child-tiers").exists():
                    _check_child_tiers(stream, home)
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


def _request(stream: socket.socket, method: str, params: dict) -> dict:
    _send(stream, {"id": "tier-check", "method": method, "params": params}, True)
    while True:
        reply = json.loads(_message(stream))
        if reply.get("id") == "tier-check":
            assert "error" not in reply, reply
            return reply["result"]


def _check_child_tiers(stream: socket.socket, home: Path) -> None:
    fresh = _request(stream, "thread/start", {})
    forked = _request(stream, "thread/fork", {"threadId": fresh["thread"]["id"]})
    (home / "native-child-tiers.json").write_text(json.dumps({
        "new": fresh["serviceTier"], "fork": forked["serviceTier"],
    }))


def _inherited(home: Path) -> dict:
    """What a host wrapper and the user's config give every app-server."""
    return {
        "layers": [{"name": {"type": "sessionFlags"}, "config": {
            "hooks": {"SessionStart": [{"matcher": "startup", "hooks": [
                {"type": "command", "command": "host-hook"}
            ]}], "state": {"host-trust": {"trusted_hash": "unchanged"}}}
        }}],
        "config": {
            "service_tier": "priority",
            "sandbox_workspace_write": {"writable_roots": [str(home / "configured-root")]}
        },
    }


def _stdio(home: Path) -> None:
    """A short-lived app-server on stdio, as lf reads inherited configuration."""
    for line in sys.stdin:
        rpc = json.loads(line)
        if "id" not in rpc:
            continue
        result = _inherited(home) if rpc["method"] == "config/read" else {}
        print(json.dumps({"id": rpc["id"], "result": result}), flush=True)


def _inline_toml(value: str) -> object:
    """lf's TOML inline values are JSON with `=` between a key and its value."""
    text, quoted, escaped = [], False, False
    for char in value:
        if quoted:
            quoted = escaped or char != '"'
            escaped = not escaped and char == "\\"
        elif char == '"':
            quoted = True
        elif char == "=":
            char = ":"
        text.append(char)
    return json.loads("".join(text))


def _overrides(args: list[str]) -> dict:
    """The `-c` values before the subcommand, as Codex reads them."""
    overrides = {}
    for flag, pair in zip(args, args[1:]):
        if flag == "-c":
            key, value = pair.split("=", 1)
            overrides[key] = _inline_toml(value)
    return overrides


def _server(endpoint: str, home: Path) -> None:
    server_config = json.loads((home / "native-server-config.json").read_text())
    tier_file = home / "native-tier.json"
    service_tier = json.loads(tier_file.read_text()) if tier_file.exists() else None
    turns_file = home / "native-turns.json"
    turns = json.loads(turns_file.read_text()) if turns_file.exists() else []
    # A server that stopped mid-turn loads that turn as interrupted.
    for turn in turns:
        if turn["status"] == "inProgress":
            turn["status"] = "interrupted"
    thread_file = home / "native-thread"
    thread = thread_file.read_text() if thread_file.exists() else ""
    lock = threading.RLock()
    # Like app-server: a thread has no rollout until it has a turn or a name,
    # every subscriber hears every event, and a client that joins mid-turn is
    # asked the approval still pending.
    named = bool(thread)
    subscribers: list[socket.socket] = []
    pending: dict | None = None

    def broadcast(value: dict) -> None:
        with lock:
            for subscriber in list(subscribers):
                try:
                    _send(subscriber, value)
                except OSError:
                    subscribers.remove(subscriber)

    def serve(stream: socket.socket) -> None:
        nonlocal thread, turns, named, pending, service_tier
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
                            pending = None
                            broadcast(
                                {
                                    "method": "turn/completed",
                                    "params": {"threadId": thread, "turn": turn},
                                }
                            )
                        continue
                    if "id" not in rpc:
                        continue
                    result = {}
                    if method in ("thread/start", "thread/resume", "thread/fork"):
                        if method in ("thread/start", "thread/fork"):
                            thread = "thread-" + uuid.uuid4().hex
                            turns = []
                            thread_file.write_text(thread)
                            service_tier = server_config.get(
                                "service_tier", _inherited(home)["config"]["service_tier"]
                            )
                        elif not (turns or named) or params.get("threadId") != thread:
                            _send(
                                stream,
                                {
                                    "id": rpc["id"],
                                    "error": {"code": -32600, "message": "no rollout found"},
                                },
                            )
                            continue
                        service_tier = params.get("serviceTier", service_tier)
                        tier_file.write_text(json.dumps(service_tier))
                        result = {"thread": {"id": thread, "turns": turns}, "serviceTier": service_tier}
                        if "cwd" in params:  # lf's own request, not a client's
                            (home / "native-config.json").write_text(json.dumps(params))
                        with lock:
                            subscribers.append(stream)
                            _send(stream, {"id": rpc["id"], "result": result})
                            if pending:
                                _send(stream, pending)
                        continue
                    elif method == "thread/settings/update":
                        service_tier = params["serviceTier"]
                        tier_file.write_text(json.dumps(service_tier))
                        result = {"serviceTier": service_tier}
                    elif method == "thread/name/set":
                        named = True
                    elif method == "thread/turns/list":
                        result = {"data": turns, "nextCursor": None}
                    elif method == "thread/loaded/list":
                        result = {"data": [thread] if thread else [], "nextCursor": None}
                    elif method == "config/read":
                        result = _inherited(home)
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
                        with lock:
                            broadcast(
                                {
                                    "method": "turn/started",
                                    "params": {"threadId": thread, "turn": turn},
                                }
                            )
                            pending = {
                                "id": "approval",
                                "method": "item/commandExecution/requestApproval",
                                "params": {"threadId": thread, "turnId": turn["id"]},
                            }
                            broadcast(pending)
            except (EOFError, OSError):
                pass
            finally:
                with lock:
                    if stream in subscribers:
                        subscribers.remove(stream)

    with socket.socket(socket.AF_UNIX) as listener:
        listener.bind(endpoint)
        listener.listen()
        while True:
            client, _ = listener.accept()
            threading.Thread(target=serve, args=(client,), daemon=True).start()


def main() -> None:
    args = sys.argv[1:]
    home = Path(os.environ["LF_HOME"])
    # A wrapper can reject long argv without ever seeing the large payload: a
    # first request never rides argv. The server's configuration, instructions
    # included, does.
    limit = 1024 if "--remote" in args else 64 * 1024
    assert all(len(arg.encode()) < limit for arg in args), args
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
            args[-1],
        )
    elif "--listen" in args:
        if (home / "fail-server").exists():
            raise SystemExit(23)
        if (home / "stall-server").exists():
            threading.Event().wait(60)
        (home / "native-server-config.json").write_text(
            json.dumps(_overrides(args[: args.index("app-server")]))
        )
        _server(args[args.index("--listen") + 1].removeprefix("unix://"), home)
    elif "app-server" in args:
        _stdio(home)
    else:
        # A bare launch is plain `codex` in its terminal, configured by argv.
        print("\n".join(args), flush=True)
        with (home / "launched").open("a") as file:
            file.write(os.environ.get("LF_CAPTURE_KEY", "") + "\n")


if __name__ == "__main__":
    main()

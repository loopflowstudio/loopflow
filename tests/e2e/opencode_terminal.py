"""Exercise the installed OpenCode TUI through lf against an isolated local API."""

import argparse
import fcntl
import json
import os
import shutil
import signal
import sqlite3
import struct
import subprocess
import tempfile
import termios
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from tui import _Tui


class _Handler(BaseHTTPRequestHandler):
    failing = True
    failures = 0
    requests: list[dict] = []

    def log_message(self, *args: object) -> None:
        pass

    def do_POST(self) -> None:
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        self.requests.append(body)
        if self.failing:
            type(self).failures += 1
            data = json.dumps(
                {"error": {"message": "Upstream request failed: Model access is disabled"}}
            ).encode()
            self.send_response(503)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        self.send_response(200)
        self.send_header(
            "Content-Type", "text/event-stream" if body.get("stream") else "application/json"
        )
        self.end_headers()
        if body.get("stream"):
            for delta, finish in [
                ({"role": "assistant", "content": "fixture response"}, None),
                ({}, "stop"),
            ]:
                chunk = {
                    "id": "chatcmpl-fixture",
                    "object": "chat.completion.chunk",
                    "created": 1,
                    "model": body["model"],
                    "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
                }
                self.wfile.write(f"data: {json.dumps(chunk)}\n\n".encode())
            self.wfile.write(b"data: [DONE]\n\n")
        else:
            self.wfile.write(
                json.dumps(
                    {
                        "id": "chatcmpl-fixture",
                        "object": "chat.completion",
                        "created": 1,
                        "model": body["model"],
                        "choices": [
                            {
                                "index": 0,
                                "message": {"role": "assistant", "content": "fixture response"},
                                "finish_reason": "stop",
                            }
                        ],
                        "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8},
                    }
                ).encode()
            )


def _wait(tui: _Tui, predicate, description: str, seconds: float = 30) -> None:
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        tui.drain(0.1)
        if predicate():
            return
        if tui.child.poll() is not None:
            break
    raise AssertionError(description + "\n" + "\n".join(tui.screen.display))


def _responses(tui: _Tui) -> int:
    return sum(line.lstrip().startswith("fixture response") for line in tui.screen.display)


def _probe(lf: Path, native: Path, dependencies: Path, output: Path, expect_overlay: bool) -> None:
    root = Path(tempfile.mkdtemp(prefix="lf-opencode-", dir="/tmp"))
    home, work = root / "home", root / "work"
    home.mkdir()
    work.mkdir()
    shutil.copytree(dependencies, home / ".config/opencode")
    (work / ".lf").mkdir()
    (work / ".lf/config.yaml").write_text(
        "agent: opencode\ndiff: false\ndiff_files: false\npaste: false\n"
    )
    subprocess.run(["git", "init", "-q", str(work)], check=True)
    subprocess.run(
        [
            "git",
            "-C",
            str(work),
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "fixture",
        ],
        check=True,
    )
    server = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    settings = {
        "model": "opencode/glm-5.2",
        "small_model": "opencode/glm-5.2",
        "autoupdate": False,
        "share": "disabled",
        "enabled_providers": ["opencode"],
        "provider": {
            "opencode": {
                "npm": "@ai-sdk/openai-compatible",
                "options": {
                    "baseURL": f"http://127.0.0.1:{server.server_port}/v1",
                    "apiKey": "local-fixture-not-a-credential",
                },
                "models": {
                    "glm-5.2": {"name": "GLM fixture", "limit": {"context": 128000, "output": 4096}}
                },
            }
        },
    }
    env = {
        "HOME": str(home),
        "LF_HOME": str(root / "lf"),
        "LF_BIN": str(lf),
        "PATH": f"{native.parent}:/usr/bin:/bin",
        "TERM": "xterm-256color",
        "LANG": "en_US.UTF-8",
        "XDG_CONFIG_HOME": str(home / ".config"),
        "XDG_DATA_HOME": str(home / ".local/share"),
        "XDG_CACHE_HOME": str(home / ".cache"),
        "XDG_STATE_HOME": str(home / ".local/state"),
        "OPENCODE_CONFIG_CONTENT": json.dumps(settings),
        "OPENCODE_DISABLE_DEFAULT_PLUGINS": "true",
        "OPENCODE_DISABLE_MODELS_FETCH": "true",
        "OPENCODE_API_KEY": "local-fixture-not-a-credential",
    }
    tui = _Tui([str(lf), "-i", "-a", "opencode", ":", "Reply with fixture response"], work, env)
    output.mkdir(parents=True, exist_ok=True)
    try:
        _wait(tui, lambda: _Handler.failures >= 1, "initial request did not reach local API")
        os.write(tui.master, b"draft survives retry")

        def stream_errors() -> int:
            evidence = (
                bytes(tui.raw)
                if expect_overlay
                else b"".join(
                    log.read_bytes() for log in (root / "lf").rglob("opencode.stderr.log")
                )
            )
            return evidence.count(b'message="stream error"')

        _wait(tui, lambda: stream_errors() >= 3, "provider did not retry")
        tui.drain(0.5)
        (output / "retry-screen.txt").write_text("\n".join(tui.screen.display))
        raw_overlay = b"level=ERROR" in tui.raw
        assert raw_overlay == expect_overlay, (
            f"raw overlay={raw_overlay}, expected={expect_overlay}"
        )
        assert "Model access is disabled" in "\n".join(tui.screen.display), (
            "native error UI missing"
        )
        if expect_overlay:
            return
        assert "draft survives retry" in "\n".join(tui.screen.display)
        logs = list((root / "lf").rglob("opencode.stderr.log"))
        assert logs and any(b"Model access is disabled" in log.read_bytes() for log in logs)
        fcntl.ioctl(tui.master, termios.TIOCSWINSZ, struct.pack("HHHH", 36, 130, 0, 0))
        tui.screen.resize(36, 130)
        os.killpg(tui.child.pid, signal.SIGWINCH)
        tui.drain(0.5)
        os.write(tui.master, b"\x7f!")
        tui.drain(0.5)
        (output / "resized-screen.txt").write_text("\n".join(tui.screen.display))
        assert "draft survives retr!" in "\n".join(tui.screen.display), (
            "draft lost or uneditable after resize"
        )
        assert b"level=ERROR" not in tui.raw
        _Handler.failing = False
        _wait(
            tui,
            lambda: _responses(tui) >= 1 and "retrying" not in "\n".join(tui.screen.display),
            "retry did not recover",
            45,
        )
        os.write(tui.master, b"\r")
        _wait(
            tui,
            lambda: any("draft survives retr!" in json.dumps(body) for body in _Handler.requests),
            "edited draft was not submitted",
        )
        _wait(tui, lambda: _responses(tui) >= 2, "edited draft did not complete")
        (output / "success-screen.txt").write_text("\n".join(tui.screen.display))
        os.write(tui.master, b"\x03")
        _wait(tui, lambda: tui.child.poll() is not None, "terminal did not exit")
        assert tui.child.returncode == 0
        with sqlite3.connect(root / "lf/loopflow.db") as db:
            session = db.execute(
                "SELECT session_id, json_extract(payload, '$.evidence.provider_session_id') "
                "FROM session_events WHERE json_extract(payload, '$.source') "
                "LIKE 'provider-session:%' LIMIT 1"
            ).fetchone()
        assert session and session[1].startswith("ses_")
        (output / "session.json").write_text(json.dumps(session))
        successful = _Tui([str(lf), "-i", "-a", "opencode", ":", "Normal launch"], work, env)
        try:
            _wait(successful, lambda: _responses(successful) >= 1, "normal launch did not complete")
            assert b"level=ERROR" not in successful.raw
            (output / "normal-screen.txt").write_text("\n".join(successful.screen.display))
            os.write(successful.master, b"\x03")
            _wait(
                successful,
                lambda: successful.child.poll() is not None,
                "normal launch did not exit",
            )
            assert successful.child.returncode == 0
        finally:
            (output / "normal-terminal.raw").write_bytes(successful.raw)
            successful.close()
    finally:
        (output / "terminal.raw").write_bytes(tui.raw)
        (output / "screen.txt").write_text("\n".join(tui.screen.display))
        (output / "requests.json").write_text(json.dumps(_Handler.requests, indent=2))
        for index, log in enumerate((root / "lf").rglob("opencode.stderr.log")):
            shutil.copyfile(log, output / f"diagnostics-{index}.log")
        tui.close()
        server.shutdown()
        shutil.rmtree(root)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lf", type=Path, required=True)
    parser.add_argument("--native", type=Path, required=True)
    parser.add_argument("--dependencies", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expect-overlay", action="store_true")
    args = parser.parse_args()
    _probe(
        args.lf.resolve(),
        args.native.resolve(),
        args.dependencies.resolve(),
        args.output.resolve(),
        args.expect_overlay,
    )
    print("OpenCode terminal probe passed")


if __name__ == "__main__":
    main()

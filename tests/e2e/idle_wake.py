"""Steer an idle OpenCode or Pi work session through lf and watch it start a turn.

Real TUIs under a PTY, a private Machine and a scripted local model. Nothing
is typed after the first turn: `lf steer` is the only input.
"""

import argparse
import json
import os
import shutil
import signal
import subprocess
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from tui import _Tui

STEER = "WAKE_MARKER use the other table"


class _Model(BaseHTTPRequestHandler):
    requests: list[dict] = []

    def log_message(self, *args: object) -> None:
        pass

    def do_POST(self) -> None:
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        self.requests.append(body)

        def chunk(delta: dict, finish: str | None) -> dict:
            return {
                "id": "chatcmpl-fixture",
                "object": "chat.completion.chunk",
                "created": 1,
                "model": body["model"],
                "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
            }

        parts = [
            chunk({"role": "assistant", "content": f"fixture answer {len(self.requests)}"}, None),
            chunk({}, "stop"),
            {
                **chunk({}, None),
                "choices": [],
                "usage": {"prompt_tokens": 31, "completion_tokens": 7, "total_tokens": 38},
            },
        ]
        data = "".join(f"data: {json.dumps(part)}\n\n" for part in parts) + "data: [DONE]\n\n"
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(data.encode())))
        self.end_headers()
        self.wfile.write(data.encode())


def _heard(marker: str, since: int = 0) -> bool:
    return any(marker in json.dumps(body) for body in _Model.requests[since:])


def _owned(root: Path) -> list[int]:
    """Processes whose environment names this run's private root."""
    listing = subprocess.run(
        # -ax: the holder has no terminal, and plain `ps` lists only those that do.
        ["ps", "-axeww", "-o", "pid=,command="], capture_output=True, text=True, check=True
    ).stdout
    return [
        int(line.split(None, 1)[0])
        for line in listing.splitlines()
        if str(root) in line and int(line.split(None, 1)[0]) != os.getpid()
    ]


def _probe(lf: Path, harness: str, native: Path, output: Path) -> None:
    root = Path(tempfile.mkdtemp(prefix="lf-wake-e2e-", dir="/tmp"))
    home, work, machine = root / "home", root / "work", root / "lf"
    home.mkdir()
    work.mkdir()
    machine.mkdir()
    model = ThreadingHTTPServer(("127.0.0.1", 0), _Model)
    threading.Thread(target=model.serve_forever, daemon=True).start()
    endpoint = f"http://127.0.0.1:{model.server_port}/v1"
    env = {
        "HOME": str(home),
        "LF_HOME": str(machine),
        "LF_BIN": str(lf),
        "PATH": f"{lf.parent}:{native.parent}:/opt/homebrew/bin:/usr/bin:/bin",
        "TERM": "xterm-256color",
        "LANG": "en_US.UTF-8",
    }
    if harness == "opencode":
        agent = "opencode:opencode/glm-5.2"
        settings = {
            "model": "opencode/glm-5.2",
            "small_model": "opencode/glm-5.2",
            "autoupdate": False,
            "share": "disabled",
            "enabled_providers": ["opencode"],
            "permission": {"*": "allow"},
            "provider": {
                "opencode": {
                    "npm": "@ai-sdk/openai-compatible",
                    "options": {"baseURL": endpoint, "apiKey": "local-fixture-not-a-credential"},
                    "models": {
                        "glm-5.2": {
                            "name": "fixture",
                            "tool_call": True,
                            "limit": {"context": 128000, "output": 4096},
                        }
                    },
                }
            },
        }
        # A held terminal starts from the account's files, not this shell.
        (home / ".config/opencode").mkdir(parents=True)
        (home / ".config/opencode/opencode.json").write_text(json.dumps(settings))
        env |= {
            "OPENCODE_DISABLE_DEFAULT_PLUGINS": "true",
            "OPENCODE_DISABLE_MODELS_FETCH": "true",
            "OPENCODE_API_KEY": "local-fixture-not-a-credential",
        }
    else:
        agent = "pi:fixture/proof"
        directory = home / ".pi/agent"
        directory.mkdir(parents=True)
        (directory / "models.json").write_text(
            json.dumps(
                {
                    "providers": {
                        "fixture": {
                            "baseUrl": endpoint,
                            "api": "openai-completions",
                            "apiKey": "fixture-not-a-secret",
                            "models": [{"id": "proof", "contextWindow": 200000}],
                        }
                    }
                }
            )
        )
        (directory / "settings.json").write_text(
            json.dumps(
                {"retry": {"enabled": False}, "defaultProvider": "fixture", "defaultModel": "proof"}
            )
        )
        env |= {"PI_OFFLINE": "1"}

    def git(*args: str) -> None:
        subprocess.run(["git", "-C", str(work), *args], check=True, env=env)

    subprocess.run(["git", "init", "-q", "--bare", str(root / "origin.git")], check=True)
    subprocess.run(["git", "init", "-q", "-b", "main", str(work)], check=True)
    git("config", "user.name", "Fixture")
    git("config", "user.email", "fixture@example.invalid")
    (work / ".lf").mkdir()
    (work / ".lf/config.yaml").write_text(
        f"agent: {agent}\ndiff: false\ndiff_files: false\npaste: false\n"
    )
    git("add", "-A")
    git("commit", "-qm", "fixture")
    git("remote", "add", "origin", str(root / "origin.git"))
    git("push", "-q", "-u", "origin", "main")

    def run(*args: str) -> str:
        done = subprocess.run(
            [str(lf), *args],
            cwd=work,
            env=env,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
        )
        assert done.returncode == 0, f"lf {' '.join(args)}\n{done.stdout}\n{done.stderr}"
        return done.stdout

    shutil.rmtree(output, ignore_errors=True)
    output.mkdir(parents=True)
    server = subprocess.Popen(
        [str(lf), "server", "run"],
        cwd=work,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=(output / "server.log").open("wb"),
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    tui = None
    try:
        created = run("task", "create", "Idle wake fixture", "--json")
        (output / "task.json").write_text(created)
        task = json.loads(created)["task"]["id"]
        run("checkout", task)
        tui = _Tui([str(lf), "resume", "--task", task], work, env)
        # The work session opens on its skill; nothing is typed to start it.
        tui.wait(lambda: len(_Model.requests) >= 1, "the first turn never reached the model", 45)
        # Let the turn end and its Stop reach the server.
        tui.drain(6.0)
        before = len(_Model.requests)
        run("steer", task, STEER)
        tui.wait(lambda: _heard("WAKE_MARKER", before), "the idle work session was not woken", 45)
        tui.drain(2.0)
        (output / "screen.txt").write_text("\n".join(tui.screen.display))
        typed = len(_Model.requests)
        tui.enter("typed after wake")
        tui.wait(lambda: _heard("typed after wake", typed), "typing after a wake ran no turn", 45)
    finally:
        (output / "requests.json").write_text(json.dumps(_Model.requests, indent=2))
        if tui is not None:
            (output / "terminal.raw").write_bytes(tui.raw)
            tui.close()
        if server.poll() is None:
            os.killpg(server.pid, signal.SIGTERM)
            server.wait(timeout=10)
        # The holder and its provider outlive the terminal.
        for pid in _owned(root):
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        for log in machine.rglob("*.log"):
            shutil.copyfile(log, output / log.name)
        model.shutdown()
        shutil.rmtree(root, ignore_errors=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lf", type=Path, required=True)
    parser.add_argument("--harness", choices=["opencode", "pi"], required=True)
    parser.add_argument("--native", type=Path, required=True, help="the opencode or pi binary")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    _probe(args.lf.resolve(), args.harness, args.native, args.output.resolve())
    print(f"{args.harness} idle wake passed")


if __name__ == "__main__":
    main()

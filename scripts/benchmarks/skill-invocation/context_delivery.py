"""Observe native hook channels and compaction against credential-free local APIs."""

import argparse
import json
import os
import queue
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from launch import _codex_response
from request_mapping import _marker_locations, _response, _run


class Requests(ThreadingHTTPServer):
    def __init__(self, provider: str) -> None:
        super().__init__(("127.0.0.1", 0), Handler)
        self.provider = provider
        self.bodies: list[dict] = []

    def __enter__(self) -> "Requests":
        threading.Thread(target=self.serve_forever, daemon=True).start()
        return self

    def __exit__(self, *args: object) -> None:
        self.shutdown()
        super().__exit__(*args)


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format: str, *args: object) -> None:
        pass

    def do_POST(self) -> None:
        self.connection.settimeout(5)
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        path = "/v1/messages" if self.server.provider == "claude" else "/v1/responses"
        if self.path.split("?")[0] != path:
            self.send_error(404)
            return
        self.server.bodies.append(body)
        data = (
            _response(body["model"])
            if self.server.provider == "claude"
            else _codex_response(Path("."), True, "exec_command")
        )
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def _hook(root: Path) -> str:
    script = root / "hook.py"
    script.write_text(
        "import json, sys\nfrom pathlib import Path\n"
        "event = json.load(sys.stdin)\n"
        f"with open({str(root / 'hooks.jsonl')!r}, 'a') as output:\n"
        "    output.write(json.dumps(event) + '\\n')\n"
        f"context = Path({str(root / 'current.txt')!r}).read_text()\n"
        "print(json.dumps({'hookSpecificOutput': {\n"
        "    'hookEventName': event['hook_event_name'],\n"
        "    'additionalContext': context + '_' + event['hook_event_name']}}))\n"
    )
    return shlex.join([sys.executable, str(script)])


def _claude(root: Path, env: dict[str, str], port: int) -> None:
    settings = root / "settings.json"
    settings.write_text(
        json.dumps(
            {
                "hooks": {
                    "SessionStart": [
                        {
                            # Exclude resume so it cannot masquerade as compaction refresh.
                            "matcher": "startup|compact",
                            "hooks": [{"type": "command", "command": _hook(root)}],
                        }
                    ]
                }
            }
        )
    )
    env.update(
        CLAUDE_CONFIG_DIR=str(root / "home/.claude"),
        ANTHROPIC_API_KEY="local-fixture-not-a-credential",
        ANTHROPIC_BASE_URL=f"http://127.0.0.1:{port}",
        CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1",
        DISABLE_AUTOUPDATER="1",
    )
    executable = str(Path(shutil.which("claude")).resolve())
    base = [
        executable,
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--settings",
        str(settings),
        "--setting-sources",
        "",
        "--model",
        "claude-sonnet-4-6",
    ]
    session = str(uuid.uuid4())
    for phase, args in [
        ("start", ["--session-id", session, "LOO444_REQUEST"]),
        ("compact", ["--resume", session, "/compact"]),
        ("after", ["--resume", session, "LOO444_AFTER_REQUEST"]),
    ]:
        if phase == "compact":
            (root / "current.txt").write_text("LOO444_FRESH_CONTEXT")
        result = _run(base + args, root / "work", env)
        (root / f"{phase}.stdout").write_text(result.stdout)
        (root / f"{phase}.stderr").write_text(result.stderr)
        assert result.returncode == 0, f"Claude {phase} failed: see {root}"


def _codex(root: Path, env: dict[str, str], port: int) -> None:
    native = root / "home/.codex"
    native.mkdir()
    env["CODEX_HOME"] = str(native)
    command = json.dumps(_hook(root))
    (native / "config.toml").write_text(f"""model = "gpt-5.4"
model_provider = "fixture"
cli_auth_credentials_store = "file"
allow_login_shell = false
sandbox_mode = "danger-full-access"
approval_policy = "never"
[[hooks.SessionStart]]
[[hooks.SessionStart.hooks]]
type = "command"
command = {command}
[[hooks.PostCompact]]
[[hooks.PostCompact.hooks]]
type = "command"
command = {command}
[features]
shell_snapshot = false
[model_providers.fixture]
name = "Local fixture"
base_url = "http://127.0.0.1:{port}/v1"
wire_api = "responses"
requires_openai_auth = false
[analytics]
enabled = false
[feedback]
enabled = false
""")
    (root / "work/AGENTS.md").write_text("LOO444_REPO_GUIDE")
    executable = str(Path(shutil.which("codex")).resolve())
    messages: queue.Queue = queue.Queue()
    transcript = []
    with (root / "stderr").open("w") as errors:
        process = subprocess.Popen(
            [executable, "app-server"],
            cwd=root / "work",
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=errors,
            text=True,
            start_new_session=True,
        )

        def read() -> None:
            for line in process.stdout:
                message = json.loads(line)
                transcript.append(message)
                messages.put(message)

        reader = threading.Thread(target=read, daemon=True)
        reader.start()
        sequence = 0

        def wait(predicate) -> dict:
            deadline = time.monotonic() + 30
            while (remaining := deadline - time.monotonic()) > 0:
                message = messages.get(timeout=remaining)
                if predicate(message):
                    return message
            raise TimeoutError(f"Codex response deadline exceeded: see {root}")

        def call(method: str, params: dict) -> dict:
            nonlocal sequence
            sequence += 1
            process.stdin.write(
                json.dumps({"id": sequence, "method": method, "params": params}) + "\n"
            )
            process.stdin.flush()
            message = wait(lambda item: item.get("id") == sequence)
            assert "error" not in message, message
            return message["result"]

        try:
            call("initialize", {"clientInfo": {"name": "context-probe", "version": "1"}})
            process.stdin.write('{"method":"initialized"}\n')
            process.stdin.flush()
            listed = call("hooks/list", {"cwds": [str(root / "work")]})
            hooks = listed["data"][0]["hooks"]
            assert len(hooks) == 2
            # Trust only this fixture's two definitions, for this thread. No saved
            # user trust or settings are changed; app-server ignores the TUI flag.
            trust = {hook["key"]: {"trusted_hash": hook["currentHash"]} for hook in hooks}
            result = call(
                "thread/start",
                {
                    "cwd": str(root / "work"),
                    "developerInstructions": "LOO444_FIXED_ADDITION",
                    "config": {"hooks": {"state": trust}},
                },
            )
            thread = result["thread"]["id"]

            def turn(text: str) -> None:
                call("turn/start", {"threadId": thread, "input": [{"type": "text", "text": text}]})
                wait(lambda item: item.get("method") == "turn/completed")

            turn("LOO444_REQUEST")
            (root / "current.txt").write_text("LOO444_FRESH_CONTEXT")
            call("thread/compact/start", {"threadId": thread})
            wait(lambda item: item.get("method") == "turn/completed")
            turn("LOO444_AFTER_REQUEST")
        finally:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)
            reader.join(timeout=5)
            (root / "messages.json").write_text(json.dumps(transcript, indent=2))


def _assess(provider: str, requests: list[dict], events: list[dict]) -> dict[str, bool]:
    first, last = requests[0], requests[-1]
    role = "user" if provider == "claude" else "developer"
    checks = {
        "startup_context_conversation_only": _marker_locations(first, "LOO444_START_CONTEXT")
        == [role],
        "fresh_context_conversation_only": _marker_locations(
            last, "LOO444_FRESH_CONTEXT_SessionStart"
        )
        == [role],
        "old_context_compacted": not _marker_locations(last, "LOO444_START_CONTEXT"),
        "startup_hook_ran": any(event.get("source") == "startup" for event in events),
        "compact_hook_ran": any(event.get("source") == "compact" for event in events),
    }
    if provider == "codex":
        checks.update(
            post_compact_hook_ran=any(
                event["hook_event_name"] == "PostCompact" for event in events
            ),
            post_compact_does_not_inject=not _marker_locations(
                last, "LOO444_FRESH_CONTEXT_PostCompact"
            ),
            additive_instructions_survive=all(
                _marker_locations(body, "LOO444_FIXED_ADDITION") == ["developer"]
                for body in [first, last]
            ),
            native_base_preserved=all(
                bool(body["instructions"]) and "LOO444" not in body["instructions"]
                for body in [first, last]
            ),
            repo_guide_user_only=all(
                _marker_locations(body, "LOO444_REPO_GUIDE") == ["user"] for body in [first, last]
            ),
        )
    return checks


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--provider", choices=["claude", "codex"], required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix=f"{args.provider}-", dir=args.output)).resolve()
    for name in ["work", "home"]:
        (root / name).mkdir()
    (root / "current.txt").write_text("LOO444_START_CONTEXT")
    env = {"PATH": os.environ["PATH"], "HOME": str(root / "home")}
    server = Requests(args.provider)
    try:
        with server:
            (_claude if args.provider == "claude" else _codex)(root, env, server.server_port)
            events = [json.loads(line) for line in (root / "hooks.jsonl").read_text().splitlines()]
            checks = _assess(args.provider, server.bodies, events)
            result = {"provider": args.provider, "checks": checks, "evidence": str(root)}
            (root / "result.json").write_text(json.dumps(result, indent=2))
            print(json.dumps(result, indent=2))
            return 0 if all(checks.values()) else 1
    finally:
        (root / "requests.json").write_text(json.dumps(server.bodies, indent=2))


if __name__ == "__main__":
    sys.exit(main())

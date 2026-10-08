"""Inspect Claude's native skill/context request mapping against a local fake API."""

import argparse
import hashlib
import json
import os
import shlex
import shutil
import signal
import subprocess
import tempfile
import threading
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from probe import _claude_command, _hook_settings, _output_events, _read_output, _user_message


def _marker_locations(request: dict, marker: str) -> list[str]:
    locations = []
    if marker in json.dumps(request.get("system", [])):
        locations.append("system")
    for message in request.get("messages", []):
        if marker in json.dumps(message.get("content", [])):
            locations.append(message["role"])
    return locations


def _assess(requests: list[dict], skill: Path, marker: str, context: str, argument: str) -> dict:
    locations = [_marker_locations(body, context) for body in requests]
    users = [_user_texts(body) for body in requests]
    return {
        "requests": len(requests),
        "request_models": sorted({body["model"] for body in requests}),
        "context_roles": locations,
        "checks": {
            "single_model_request": len(requests) == 1,
            "context_user_only": bool(locations)
            and all(roles and set(roles) == {"user"} for roles in locations),
            "selected_source": bool(users)
            and all(
                any(f"Base directory for this skill: {skill.parent}\n" in text for text in texts)
                for texts in users
            ),
            "expanded_argument": bool(users)
            and all(any(f"{marker}|{argument}|" in text for text in texts) for texts in users),
        },
    }


def _user_texts(request: dict) -> list[str]:
    texts = []
    for message in request.get("messages", []):
        if message["role"] != "user":
            continue
        content = message["content"]
        if isinstance(content, str):
            texts.append(content)
        else:
            texts.extend(block.get("text", "") for block in content)
    return texts


def _response(model: str) -> bytes:
    message = {
        "id": "msg_fixture",
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": [],
        "stop_reason": None,
        "stop_sequence": None,
        "usage": {"input_tokens": 1, "output_tokens": 1},
    }
    events = [
        {"type": "message_start", "message": message},
        {"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}},
        {
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": "fixture response"},
        },
        {"type": "content_block_stop", "index": 0},
        {
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn", "stop_sequence": None},
            "usage": {"output_tokens": 1},
        },
        {"type": "message_stop"},
    ]
    return "".join(
        f"event: {event['type']}\ndata: {json.dumps(event)}\n\n" for event in events
    ).encode()


def _run(
    command: list[str], root: Path, env: dict[str, str], messages: list[dict]
) -> subprocess.CompletedProcess:
    with subprocess.Popen(
        command,
        cwd=root,
        env=env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    ) as process:
        try:
            stdout, stderr = process.communicate(
                "".join(json.dumps(message) + "\n" for message in messages), timeout=45
            )
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate(timeout=5)
            raise
        return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def _probe(claude: str, model: str, channel: str) -> bool:
    requests = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format: str, *args) -> None:
            pass

        def do_POST(self) -> None:
            self.connection.settimeout(5)
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            if self.path.split("?")[0] != "/v1/messages":
                self.send_error(404)
                return
            requests.append(body)
            response = _response(body["model"])
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(response)))
            self.end_headers()
            self.wfile.write(response)

    with tempfile.TemporaryDirectory(prefix="lf-request-mapping-") as temporary:
        root = Path(temporary).resolve()
        home = root / "home"
        home.mkdir()
        workspace = root / "workspace"
        workspace.mkdir()
        skill = workspace / ".claude/skills/lf-mapping/SKILL.md"
        skill.parent.mkdir(parents=True)
        skill_marker, context_marker = uuid.uuid4().hex, uuid.uuid4().hex
        skill.write_text(
            "---\nname: lf-mapping\ndescription: Local request mapping fixture.\n"
            "disable-model-invocation: true\n---\n"
            f"{skill_marker}|$ARGUMENTS|\n"
        )
        hook = root / "context.json"
        hook.write_text(
            json.dumps(
                {
                    "hookSpecificOutput": {
                        "hookEventName": "UserPromptSubmit",
                        "additionalContext": context_marker,
                    }
                }
            )
        )
        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        # An allowlist prevents inherited provider credentials, routes and lf authority.
        env = {key: os.environ[key] for key in ("PATH", "TMPDIR", "LANG") if key in os.environ}
        env.update(
            {
                "HOME": str(home),
                "CLAUDE_CONFIG_DIR": str(home / ".claude"),
                "ANTHROPIC_BASE_URL": f"http://127.0.0.1:{server.server_port}",
                "ANTHROPIC_API_KEY": "local-fixture-not-a-credential",
                "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1",
                "DISABLE_AUTOUPDATER": "1",
            }
        )
        session = str(uuid.uuid4())
        passed = True
        try:
            version = subprocess.run(
                [claude, "--version"],
                env=env,
                capture_output=True,
                text=True,
                check=True,
                timeout=10,
            ).stdout.strip()
            with Path(claude).open("rb") as executable:
                digest = hashlib.file_digest(executable, "sha256").hexdigest()
            print(json.dumps({"version": version, "executable_sha256": digest}), flush=True)
            base = _claude_command(claude) + ["--model", model]
            for index, argument in enumerate(("alpha", "beta")):
                command = base + ["--session-id" if index == 0 else "--resume", session]
                messages = [_user_message([f"/lf-mapping {argument}"])]
                if index == 0:
                    if channel == "hook":
                        command += ["--settings", _hook_settings("cat " + shlex.quote(str(hook)))]
                    else:
                        context_message = _user_message([context_marker])
                        context_message["shouldQuery"] = False
                        if channel == "staged":
                            seeded = _run(command, workspace, env, [context_message])
                            print(
                                json.dumps(
                                    {
                                        "case": "seed",
                                        "exit": seeded.returncode,
                                        "requests": len(requests),
                                    }
                                ),
                                flush=True,
                            )
                            passed &= seeded.returncode == 0 and not requests
                            command = base + ["--resume", session]
                        else:
                            messages.insert(0, context_message)
                else:
                    hook.unlink()
                start = len(requests)
                result = _run(command, workspace, env, messages)
                _, native_arguments = _read_output(_output_events(result.stdout))
                observation = _assess(
                    requests[start:], skill, skill_marker, context_marker, argument
                )
                observation["checks"].update(
                    exit_zero=result.returncode == 0,
                    native_arguments_exact=native_arguments == argument,
                )
                print(
                    json.dumps(
                        {
                            "case": "initial" if index == 0 else "resumed",
                            "model": model,
                            "channel": channel,
                            **observation,
                        }
                    ),
                    flush=True,
                )
                passed &= all(observation["checks"].values())
        finally:
            server.shutdown()
            server.server_close()
            thread.join()
        return passed


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--claude", default=shutil.which("claude"))
    parser.add_argument("--model", default="sonnet")
    parser.add_argument("--channel", choices=("hook", "queued", "staged"), default="queued")
    args = parser.parse_args()
    if not args.claude:
        parser.error("a Claude executable is required")
    return 0 if _probe(str(Path(args.claude).resolve()), args.model, args.channel) else 1


if __name__ == "__main__":
    raise SystemExit(main())

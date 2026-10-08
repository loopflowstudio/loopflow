"""Inspect Claude's native skill/context request mapping against a local fake API."""

import argparse
import errno
import fcntl
import hashlib
import json
import os
import pty
import re
import select
import shlex
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
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
            and all(set(roles) == {"user"} for roles in locations),
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


class _Terminal:
    def __init__(self, command: list[str], root: Path, env: dict[str, str]) -> None:
        self.master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
        self.process = subprocess.Popen(
            command,
            cwd=root,
            env={**env, "TERM": "xterm-256color"},
            stdin=slave,
            stdout=slave,
            stderr=slave,
            start_new_session=True,
        )
        os.close(slave)
        self._query_tail = b""

    def pump(self, seconds: float) -> None:
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            ready, _, _ = select.select([self.master], [], [], min(0.1, seconds))
            if ready:
                try:
                    data = os.read(self.master, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        return
                    raise
                if not data:
                    return
                # Answer each cursor query once, including queries split across reads.
                data = self._query_tail + data
                for _ in range(data.count(b"\x1b[6n")):
                    self.write(b"\x1b[1;1R")
                self._query_tail = data[-3:]
            if self.process.poll() is not None:
                return

    def write(self, value: bytes) -> None:
        while value:
            written = os.write(self.master, value)
            if not written:
                raise RuntimeError("terminal input closed")
            value = value[written:]

    def close(self) -> bool:
        try:
            if self.process.poll() is None:
                self.write(b"\x15/exit\r")
                self.pump(3)
            return self.process.poll() == 0
        finally:
            if self.process.poll() is None:
                os.killpg(self.process.pid, signal.SIGKILL)
            self.process.wait(timeout=5)
            os.close(self.master)


def _await_request(terminal: _Terminal, requests: list[dict], count: int) -> None:
    deadline = time.monotonic() + 20
    while len(requests) < count and terminal.process.poll() is None:
        if time.monotonic() >= deadline:
            raise TimeoutError(f"expected {count} requests, received {len(requests)}")
        terminal.pump(0.1)
    if len(requests) < count:
        raise RuntimeError(f"terminal exited {terminal.process.returncode} before request {count}")
    terminal.pump(0.5)


def _send_inbox(message_path: Path) -> None:
    # The fixture's SessionStart child consumes its own provider-issued local
    # capability directly from its environment. Never record or print the token.
    deadline = time.monotonic() + 45
    while not message_path.exists():
        if not message_path.parent.exists():
            return
        if time.monotonic() >= deadline:
            raise TimeoutError("no inbox message supplied")
        time.sleep(0.05)
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(5)
        connection.connect(os.environ["CLAUDE_CODE_MESSAGING_SOCKET"])
        frames = [
            {"type": "auth", "token": os.environ["CLAUDE_CODE_MESSAGING_TOKEN"]},
            {"type": "user", "message": {"role": "user", "content": message_path.read_text()}},
        ]
        connection.sendall("".join(json.dumps(frame) + "\n" for frame in frames).encode())


def _terminal_mapping(
    claude: str,
    workspace: Path,
    env: dict[str, str],
    model: str,
    session: str,
    requests: list[dict],
    marker: str,
) -> dict:
    root = workspace.parent
    socket_path = root / "inbox.sock"
    inbox_message = root / "inbox-message.txt"
    draft = "UNSUBMITTED_DRAFT_" + uuid.uuid4().hex
    config = {
        "hasCompletedOnboarding": True,
        "theme": "dark",
        "customApiKeyResponses": {"approved": [env["ANTHROPIC_API_KEY"][-20:]], "rejected": []},
        "projects": {str(workspace): {"hasTrustDialogAccepted": True}},
    }
    for path in (
        Path(env["HOME"]) / ".claude.json",
        Path(env["CLAUDE_CONFIG_DIR"]) / ".claude.json",
    ):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(config))
    hook = {
        "type": "command",
        "async": True,
        "command": shlex.join(
            [
                sys.executable,
                str(Path(__file__).resolve()),
                "--inbox-message",
                str(inbox_message),
            ]
        ),
    }
    settings = {"hooks": {"SessionStart": [{"hooks": [hook]}]}}
    terminal = _Terminal(
        [
            claude,
            "--setting-sources",
            "project",
            "--strict-mcp-config",
            "--mcp-config",
            '{"mcpServers":{}}',
            "--tools",
            "",
            "--model",
            model,
            "--resume",
            session,
            "--messaging-socket-path",
            str(socket_path),
            "--settings",
            json.dumps(settings),
            "--",
            "/lf-mapping alpha",
        ],
        workspace,
        env,
    )
    try:
        _await_request(terminal, requests, 1)
        # A person has a draft in the native editor when an external writer
        # injects its command. Only this probe's own PTY receives input.
        terminal.write(draft.encode())
        terminal.pump(0.5)
        terminal.write(b"/lf-mapping beta\r")
        _await_request(terminal, requests, 2)
        draft_users = _user_texts(requests[-1])
        inbox_bound = socket_path.exists()
        if inbox_bound:
            inbox_message.write_text("/lf-mapping gamma")
            _await_request(terminal, requests, 3)
        inbox_users = _user_texts(requests[-1])
    finally:
        clean_exit = terminal.close()
    return {
        "requests": len(requests),
        "checks": {
            "draft_was_submitted": any(draft + "/lf-mapping beta" in text for text in draft_users),
            "injected_skill_not_expanded": not any(
                marker + "|beta|" in text for text in draft_users
            ),
            "inbox_bound": inbox_bound,
            "inbox_delivered_text": any("/lf-mapping gamma" in text for text in inbox_users),
            "inbox_skill_not_expanded": not any(marker + "|gamma|" in text for text in inbox_users),
            "exact_request_count": len(requests) == 3,
            "same_native_session": all(
                session in json.dumps(body.get("metadata", {})) for body in requests
            ),
            "clean_terminal_exit": clean_exit,
        },
    }


def _probe(
    claude: str, model: str, channel: str, lf: str | None = None, flow: bool = False
) -> bool:
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

    with tempfile.TemporaryDirectory(prefix="lf-request-", dir="/tmp") as temporary:
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
            f"{skill_marker}|$ARGUMENTS|\nasset-path: ${{CLAUDE_SKILL_DIR}}/reference.txt\n"
        )
        (skill.parent / "reference.txt").write_text("fixture bundled reference")
        original_source = skill.read_text()
        plugin = root / "selected-plugin"
        if channel in ("plugin", "command-plugin", "snapshot-plugin"):
            (plugin / ".claude-plugin").mkdir(parents=True)
            (plugin / ".claude-plugin/plugin.json").write_text(json.dumps({"name": "lf-selected"}))
            (plugin / "skills").mkdir()
            if channel == "snapshot-plugin":
                selected = plugin / "skills/invoke"
                selected.mkdir()
                (selected / "SKILL.md").write_text(skill.read_text())
                (selected / "reference.txt").symlink_to(skill.parent / "reference.txt")
            else:
                (plugin / "skills/lf-mapping").symlink_to(skill.parent, target_is_directory=True)
            if channel == "command-plugin":
                (plugin / "SKILL.md").write_text(skill.read_text())
                (plugin / "reference.txt").symlink_to(skill.parent / "reference.txt")
                (plugin / ".claude-plugin/plugin.json").write_text(
                    json.dumps(
                        {
                            "name": "lf-selected",
                            "commands": {"invoke": {"source": "./SKILL.md"}},
                        }
                    )
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
            if lf:
                context_file = workspace / "context.md"
                context_file.write_text(context_marker)
                fixture_bin = root / "bin"
                fixture_bin.mkdir()
                wrapper = fixture_bin / "claude"
                wrapper.write_text(
                    "#!/bin/sh\nANTHROPIC_API_KEY=local-fixture-not-a-credential exec "
                    + shlex.quote(claude)
                    + ' "$@"\n'
                )
                wrapper.chmod(0o755)
                env.update({"LF_HOME": str(root / "lf"), "LF_BIN": lf})
                env["PATH"] = str(fixture_bin) + os.pathsep + env["PATH"]
                target = ["lf-mapping", "alpha"]
                if flow:
                    flows = workspace / ".lf/flows"
                    flows.mkdir(parents=True)
                    (flows / "mapping.yaml").write_text("- lf-mapping\n")
                    child = fixture_bin / "lf"
                    child.write_text(
                        "#!/bin/sh\nrm -f "
                        + shlex.quote(str(skill))
                        + "\nexec "
                        + shlex.quote(lf)
                        + ' "$@"\n'
                    )
                    child.chmod(0o755)
                    env["LF_BIN"] = str(child)
                    # A newly visible collision must not replace the captured source.
                    collision = home / ".claude/skills/lf-mapping/SKILL.md"
                    collision.parent.mkdir(parents=True)
                    collision.write_text("---\ndescription: Wrong source\n---\nWRONG_SOURCE\n")
                    target = ["flow", "mapping", "alpha"]
                result = _run(
                    [
                        lf,
                        "-b",
                        "--no-loopflow",
                        "--agent",
                        f"claude:{model}",
                        "--docs",
                        str(context_file),
                        *target,
                    ],
                    workspace,
                    env,
                    [],
                )
                snapshots = list((root / "lf/runs").glob("*/*/skill-*/skills/invoke/SKILL.md"))
                selected = snapshots[0] if len(snapshots) == 1 else skill
                observation = _assess(requests, selected, skill_marker, context_marker, "alpha")
                manifests = [
                    json.loads(path.read_text())
                    for path in (root / "lf/runs").glob("*/*/manifest.json")
                ]
                origins = [
                    manifest.get("exec", {}).get("skill_invocation") for manifest in manifests
                ]
                events = [
                    json.loads(line)
                    for path in (root / "lf/runs").glob("*/*/events.jsonl")
                    for line in path.read_text().splitlines()
                ]
                raw = "\n".join(
                    event["line"]
                    for event in events
                    if event.get("type") == "provider_output" and event.get("stream") == "stdout"
                )
                _, native_arguments = _read_output(_output_events(raw))
                observation["checks"].update(
                    exit_zero=result.returncode == 0,
                    native_arguments_exact=native_arguments == "alpha",
                    selected_bytes_retained=len(snapshots) == 1
                    and selected.read_text() == original_source,
                    bundled_reference_reachable=len(snapshots) == 1
                    and (selected.parent / "reference.txt").read_text()
                    == "fixture bundled reference",
                    origin_retained=any(
                        origin
                        and origin["skill"]["source"]["path"] == str(skill)
                        and origin["arguments"] == "alpha"
                        for origin in origins
                    ),
                )
                if flow:
                    observation["checks"]["removed_source_not_reselected"] = (
                        not skill.exists() and "WRONG_SOURCE" not in json.dumps(requests)
                    )
                print(
                    json.dumps({"channel": "lf-flow" if flow else "lf", **observation}), flush=True
                )
                if result.returncode:
                    print(result.stderr, file=sys.stderr)
                    for event in _output_events(raw):
                        if event.get("is_error"):
                            print(event.get("result"), file=sys.stderr)
                return all(observation["checks"].values())
            base = _claude_command(claude) + ["--model", model]
            if channel in ("plugin", "command-plugin", "snapshot-plugin"):
                base += ["--plugin-dir", str(plugin)]
            command_name = "lf-selected:lf-mapping" if channel == "plugin" else "lf-mapping"
            if channel in ("command-plugin", "snapshot-plugin"):
                command_name = "lf-selected:invoke"
            initial = base + ["--session-id", session]
            resume = base + ["--resume", session]
            context_messages = []
            if channel == "hook":
                initial += ["--settings", _hook_settings("cat " + shlex.quote(str(hook)))]
            else:
                context_message = _user_message([context_marker])
                context_message["shouldQuery"] = False
                if channel in ("staged", "terminal"):
                    seeded = _run(initial, workspace, env, [context_message])
                    print(
                        json.dumps(
                            {"case": "seed", "exit": seeded.returncode, "requests": len(requests)}
                        ),
                        flush=True,
                    )
                    passed &= seeded.returncode == 0 and not requests
                    initial = resume
                else:
                    context_messages.append(context_message)

            if channel == "terminal":
                observation = _terminal_mapping(
                    claude, workspace, env, model, session, requests, skill_marker
                )
                initial_facts = _assess(requests[:1], skill, skill_marker, context_marker, "alpha")
                observation["checks"]["native_startup"] = all(initial_facts["checks"].values())
                print(json.dumps({"channel": channel, **observation}), flush=True)
                return passed and all(observation["checks"].values())

            for case, argument, command, messages in [
                ("initial", "alpha", initial, context_messages),
                ("resumed", "beta", resume, []),
            ]:
                start = len(requests)
                result = _run(
                    command,
                    workspace,
                    env,
                    [*messages, _user_message([f"/{command_name} {argument}"])],
                )
                hook.unlink(missing_ok=True)
                _, native_arguments = _read_output(_output_events(result.stdout))
                observation = _assess(
                    requests[start:], skill, skill_marker, context_marker, argument
                )
                observation["checks"].update(
                    exit_zero=result.returncode == 0,
                    native_arguments_exact=native_arguments == argument,
                )
                observation["native_paths"] = [
                    match
                    for body in requests[start:]
                    for text in _user_texts(body)
                    for match in re.findall(
                        r"(?:Base directory for this skill: |asset-path: )([^\n]+)", text
                    )
                ]
                print(
                    json.dumps(
                        {
                            "case": case,
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
    parser.add_argument(
        "--lf", type=Path, help="Exercise headless native dispatch through this lf binary"
    )
    parser.add_argument(
        "--flow",
        action="store_true",
        help="Remove the selected source after Flow capture, with a same-name collision",
    )
    parser.add_argument(
        "--channel",
        choices=(
            "hook",
            "queued",
            "staged",
            "terminal",
            "plugin",
            "command-plugin",
            "snapshot-plugin",
        ),
        default="queued",
    )
    parser.add_argument("--inbox-message", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.inbox_message:
        _send_inbox(args.inbox_message)
        return 0
    if not args.claude:
        parser.error("a Claude executable is required")
    return (
        0
        if _probe(
            str(Path(args.claude).resolve()),
            args.model,
            args.channel,
            str(args.lf.resolve()) if args.lf else None,
            args.flow,
        )
        else 1
    )


if __name__ == "__main__":
    raise SystemExit(main())

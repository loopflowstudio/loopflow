"""Inspect Claude's native skill/context request mapping against a local fake API."""

import argparse
import hashlib
import json
import os
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def _output_events(output: str) -> list[dict]:
    events = []
    for line in output.splitlines():
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return events


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


def _decode_context(text: str) -> str:
    marker = "Additional user context (decode this JSON string):\n"
    if marker not in text:
        return text
    return json.JSONDecoder().raw_decode(text.split(marker, 1)[1])[0]


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
    command: list[str], root: Path, env: dict[str, str], stdin: str | None = None
) -> subprocess.CompletedProcess:
    with subprocess.Popen(
        command,
        cwd=root,
        env=env,
        stdin=subprocess.PIPE if stdin is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    ) as process:
        try:
            stdout, stderr = process.communicate(input=stdin, timeout=45)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate(timeout=5)
            raise
        return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def _probe(claude: str, model: str, lf: str, flow: bool, terminal: bool, command: bool) -> bool:
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
        skill = workspace / (
            ".claude/commands/lf-mapping.md" if command else ".claude/skills/lf-mapping/SKILL.md"
        )
        skill.parent.mkdir(parents=True)
        skill_marker, context_marker = uuid.uuid4().hex, uuid.uuid4().hex
        context_text = (
            context_marker + '\nLiteral $ARGUMENTS $1 \\$ARGUMENTS !`echo untouched` "quotes"'
        )
        skill.write_text(
            "---\nname: lf-mapping\ndescription: Local request mapping fixture.\n"
            "disable-model-invocation: true\nmodel: haiku\nallowed-tools: Read\n"
            "future-native-setting: retained\n---\n"
            f"{skill_marker}|$ARGUMENTS|\nasset-path: ${{CLAUDE_SKILL_DIR}}/reference.txt\n"
        )
        (skill.parent / "reference.txt").write_text("fixture bundled reference")
        original_source = skill.read_text()
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
            context_file = workspace / "context.md"
            context_file.write_text(context_text)
            fixture_bin = root / "bin"
            fixture_bin.mkdir()
            wrapper = fixture_bin / "claude"
            wrapper.write_text(
                "#!/bin/sh\nANTHROPIC_API_KEY=local-fixture-not-a-credential exec "
                + shlex.quote(claude)
                + (" -p --output-format stream-json --verbose" if terminal else "")
                + ' "$@"\n'
            )
            wrapper.chmod(0o755)
            env.update({"LF_HOME": str(root / "lf"), "LF_BIN": lf})
            env["PATH"] = str(fixture_bin) + os.pathsep + env["PATH"]
            argument = '"alpha beta"\nsecond line'
            target = ["lf-mapping", argument]
            collision = home / ".claude/skills/lf-mapping/SKILL.md"
            collision.parent.mkdir(parents=True)
            collision.write_text("---\ndescription: Wrong source\n---\nWRONG_SOURCE\n")
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
                target = ["flow", "mapping", argument]
            result = _run(
                [
                    lf,
                    "--tui" if terminal else "-b",
                    "--agent",
                    f"claude:{model}",
                    "--docs",
                    str(context_file),
                    *target,
                ],
                workspace,
                env,
            )
            snapshots = list((root / "lf/runs").glob("*/*/skill-*/skills/invoke/SKILL.md"))
            selected = snapshots[0] if len(snapshots) == 1 else skill
            observation = _assess(requests, selected, skill_marker, context_marker, argument)
            manifests = [
                json.loads(path.read_text())
                for path in (root / "lf/runs").glob("*/*/manifest.json")
            ]
            origins = [
                (manifest.get("exec") or {}).get("skill_invocation") for manifest in manifests
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
            provider_events = _output_events(raw)
            observation["checks"].update(
                exit_zero=result.returncode == 0,
                declared_model_applied=all("haiku" in body["model"] for body in requests),
                native_arguments_exact=any(
                    f"<command-args>{argument}</command-args>" in text
                    for request in requests
                    for text in _user_texts(request)
                ),
                context_bytes_retained=any(
                    context_text in _decode_context(text).replace("&#36;", "$")
                    for request in requests
                    for text in _user_texts(request)
                ),
                unfamiliar_declaration_reported="future-native-setting" in result.stderr,
                collision_not_selected="WRONG_SOURCE" not in json.dumps(requests),
                captured_bytes_retained=any(
                    origin
                    and (
                        "---"
                        + origin["skill"]["source"]["frontmatter"]
                        + "---\n"
                        + origin["skill"]["content"]
                    )
                    == original_source
                    for origin in origins
                ),
                bundled_reference_reachable=(skill.parent / "reference.txt").read_text()
                == "fixture bundled reference",
                origin_retained=any(
                    origin
                    and origin["skill"]["source"]["path"] == str(skill)
                    and origin["arguments"] == argument
                    for origin in origins
                ),
            )
            if terminal:
                # Native terminal captures have no headless AgentProcessRequest.
                del observation["checks"]["origin_retained"]
                observation["checks"]["captured_bytes_retained"] = (
                    skill.read_text() == original_source
                    and selected.read_text().startswith(
                        original_source.replace("${CLAUDE_SKILL_DIR}", str(skill.parent))
                    )
                )
            if flow:
                observation["checks"]["removed_source_not_reselected"] = (
                    not skill.exists() and "WRONG_SOURCE" not in json.dumps(requests)
                )
            print(json.dumps({"channel": "lf-flow" if flow else "lf", **observation}), flush=True)
            if result.returncode:
                print(result.stderr, file=sys.stderr)
                for event in provider_events:
                    if event.get("is_error"):
                        print(event.get("result"), file=sys.stderr)
            return all(observation["checks"].values())
        finally:
            server.shutdown()
            server.server_close()
            thread.join()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--claude", default=shutil.which("claude"))
    parser.add_argument("--model", default="sonnet")
    parser.add_argument("--terminal", action="store_true")
    parser.add_argument("--command", action="store_true", help="Use a single-file Claude command")
    parser.add_argument(
        "--lf",
        type=Path,
        required=True,
        help="Exercise headless native dispatch through this lf binary",
    )
    parser.add_argument(
        "--flow",
        action="store_true",
        help="Remove the selected source after Flow capture, with a same-name collision",
    )
    args = parser.parse_args()
    if not args.claude:
        parser.error("a Claude executable is required")
    return (
        0
        if _probe(
            str(Path(args.claude).resolve()),
            args.model,
            str(args.lf.resolve()),
            args.flow,
            args.terminal,
            args.command,
        )
        else 1
    )


if __name__ == "__main__":
    raise SystemExit(main())

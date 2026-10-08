"""Inspect captured Codex skill paths against a credential-free local API."""

import argparse
import asyncio
import json
import os
import shutil
import signal
import tempfile
import threading
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from continuity import _request, _send, _until
from request_mapping import _run


async def _native(
    codex: str, workspace: Path, env: dict[str, str], snapshot: Path, context: str
) -> dict:
    process = await asyncio.create_subprocess_exec(
        codex,
        "app-server",
        "--listen",
        "stdio://",
        cwd=workspace,
        env=env,
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.DEVNULL,
        start_new_session=True,
    )
    try:
        await _request(
            process,
            "initialize",
            {
                "clientInfo": {"name": "lf_skill_fixture", "version": "1"},
                "capabilities": {"experimentalApi": True},
            },
        )
        await _send(process, {"method": "initialized"})
        roots = snapshot.parent.parent
        listed = await _request(
            process,
            "skills/list",
            {
                "cwds": [str(workspace)],
                "forceReload": True,
                "perCwdExtraUserRoots": [{"cwd": str(workspace), "extraUserRoots": [str(roots)]}],
            },
        )
        ignored = str(snapshot) not in json.dumps(listed)
        for register in [False, True]:
            if register:
                await _request(process, "skills/extraRoots/set", {"extraRoots": [str(roots)]})
            thread = await _request(
                process,
                "thread/start",
                {"cwd": str(workspace), "approvalPolicy": "never", "sandbox": "read-only"},
            )
            await _request(
                process,
                "turn/start",
                {
                    "threadId": thread["thread"]["id"],
                    "input": [
                        {"type": "text", "text": context},
                        {"type": "skill", "name": "audit", "path": str(snapshot)},
                        {"type": "text", "text": "$audit alpha"},
                    ],
                },
            )
            await _until(process, lambda event: event.get("method") == "turn/completed")
        await _request(process, "skills/extraRoots/set", {"extraRoots": []})
        cleared = await _request(
            process, "skills/list", {"cwds": [str(workspace)], "forceReload": True}
        )
        return {
            "documented_roots_ignored": ignored,
            "replacement_clears_root": str(snapshot) not in json.dumps(cleared),
        }
    finally:
        process.stdin.close()
        try:
            await asyncio.wait_for(process.wait(), 5)
        except TimeoutError:
            os.killpg(process.pid, signal.SIGKILL)
            await process.wait()


def _probe(lf: Path | None, codex: str) -> bool:
    requests = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format: str, *args: object) -> None:
            pass

        def do_POST(self) -> None:
            requests.append(json.loads(self.rfile.read(int(self.headers["Content-Length"]))))
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            item = {
                "type": "message",
                "id": "msg_fixture",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Fixture complete."}],
            }
            response = {"id": "resp_fixture", "status": "in_progress", "output": []}
            self._event("response.created", {"response": response})
            self._event("response.output_item.added", {"output_index": 0, "item": item})
            self._event("response.output_item.done", {"output_index": 0, "item": item})
            response.update(
                status="completed",
                output=[item],
                usage={
                    "input_tokens": 20,
                    "output_tokens": 5,
                    "total_tokens": 25,
                    "input_tokens_details": {"cached_tokens": 0},
                    "output_tokens_details": {"reasoning_tokens": 0},
                },
            )
            self._event("response.completed", {"response": response})

        def _event(self, kind: str, value: dict) -> None:
            self.wfile.write(
                f"event: {kind}\ndata: {json.dumps({'type': kind, **value})}\n\n".encode()
            )
            self.wfile.flush()

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with tempfile.TemporaryDirectory(prefix="lf-codex-skill-", dir="/tmp") as temporary:
            root = Path(temporary).resolve()
            home = root / "home"
            codex_home = home / ".codex"
            codex_home.mkdir(parents=True)
            (codex_home / "config.toml").write_text(
                'model = "gpt-5.4"\nmodel_provider = "fixture"\n'
                'cli_auth_credentials_store = "file"\nallow_login_shell = false\n'
                "[features]\nshell_snapshot = false\n"
                '[model_providers.fixture]\nname = "Local synthetic fixture"\n'
                f'base_url = "http://127.0.0.1:{server.server_port}/v1"\n'
                'wire_api = "responses"\nrequires_openai_auth = false\n'
                "[analytics]\nenabled = false\n[feedback]\nenabled = false\n"
            )
            workspace = root / "workspace"
            skill = workspace / ".agents/skills/audit/SKILL.md"
            skill.parent.mkdir(parents=True)
            marker, context = uuid.uuid4().hex, uuid.uuid4().hex
            source = (
                "---\nname: audit\ndescription: Local skill fixture.\n---\n"
                f"{marker}\nRead reference.txt.\n"
            )
            skill.write_text(source)
            (skill.parent / "reference.txt").write_text("bundled reference")
            context_file = workspace / "context.md"
            context_file.write_text(context)
            env = {key: os.environ[key] for key in ("PATH", "TMPDIR", "LANG") if key in os.environ}
            env.update(HOME=str(home), CODEX_HOME=str(codex_home), LF_HOME=str(root / "lf"))
            if lf is None:
                snapshot = root / "captured/skills/audit/SKILL.md"
                snapshot.parent.mkdir(parents=True)
                snapshot.write_text(source)
                checks = asyncio.run(_native(codex, workspace, env, snapshot, context))
                checks.update(
                    two_requests=len(requests) == 2,
                    unregistered_path_ignored=len(requests) == 2
                    and marker not in json.dumps(requests[0].get("input", [])),
                    registered_path_expanded=len(requests) == 2
                    and any(
                        f"<path>{snapshot}</path>" in json.dumps(item)
                        and marker in json.dumps(item)
                        for item in requests[1].get("input", [])
                        if item.get("role") == "user"
                    ),
                )
                print(json.dumps({"mode": "provider-counterexample", "checks": checks}), flush=True)
                return all(checks.values())
            env["LF_BIN"] = str(lf)
            result = _run(
                [
                    str(lf),
                    "-b",
                    "--no-loopflow",
                    "--agent",
                    "codex:gpt-5.4",
                    "--docs",
                    str(context_file),
                    "audit",
                    "alpha",
                ],
                workspace,
                env,
                [],
            )
            snapshots = list((root / "lf/runs").glob("*/*/skill-*/skills/invoke/SKILL.md"))
            messages = [item for request in requests for item in request.get("input", [])]
            users = [json.dumps(item) for item in messages if item.get("role") == "user"]
            checks = {
                "exit_zero": result.returncode == 0,
                "single_request": len(requests) == 1,
                "native_expansion": len(snapshots) == 1
                and any(
                    f"<path>{snapshots[0]}</path>" in text and marker in text for text in users
                ),
                "native_arguments": any("$audit alpha" in text for text in users),
                "context_user_only": any(context in text for text in users)
                and not any(
                    context in json.dumps(item) for item in messages if item.get("role") != "user"
                ),
                "retained_source": len(snapshots) == 1 and snapshots[0].read_text() == source,
                "bundled_reference": len(snapshots) == 1
                and (snapshots[0].parent / "reference.txt").read_text() == "bundled reference",
            }
            rpc = [
                json.loads(line)
                for path in (root / "lf/runs").glob("*/*/events.jsonl")
                for line in path.read_text().splitlines()
            ]
            registration = [
                event.get("line")
                for event in rpc
                if event.get("type") == "provider_output" and '"skills"' in event.get("line", "")
            ]
            print(json.dumps({"checks": checks}), flush=True)
            if not all(checks.values()):
                print(result.stderr)
                print(json.dumps({"registration": registration}))
            return all(checks.values())
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--lf",
        type=Path,
        help="Check the pending native LF implementation instead of the provider counterexample",
    )
    parser.add_argument("--codex", default=shutil.which("codex"))
    args = parser.parse_args()
    if not args.codex:
        parser.error("codex is required")
    return 0 if _probe(args.lf.resolve() if args.lf else None, args.codex) else 1


if __name__ == "__main__":
    raise SystemExit(main())

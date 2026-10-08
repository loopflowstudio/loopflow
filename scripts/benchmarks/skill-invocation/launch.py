"""Exercise installed skills through lf and real providers against local fake APIs."""

import argparse
import hashlib
import io
import json
import os
import shlex
import shutil
import tarfile
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.request import urlopen

from request_mapping import _response, _run

SOURCES = {
    "internal-comms": (
        "anthropics/skills",
        "683bc88e56f3e09ba94f7055977f3d3aa499f202",
        "skills/internal-comms",
        "examples/3p-updates.md",
    ),
    "skill-installer": (
        "openai/skills",
        "49f948faa9258a0c61caceaf225e179651397431",
        "skills/.system/skill-installer",
        "scripts/list-skills.py",
    ),
}


def _fetch(destination: Path) -> None:
    for name, (repo, revision, folder, _) in SOURCES.items():
        with urlopen(f"https://api.github.com/repos/{repo}/tarball/{revision}") as response:
            data = response.read()
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
            for member in archive.getmembers():
                relative = Path(member.name).parts[1:]
                prefix = Path(folder).parts
                if not member.isfile() or relative[: len(prefix)] != prefix:
                    continue
                target = destination / name / Path(*relative[len(prefix) :])
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(archive.extractfile(member).read())


def _claude_tool_response(model: str, path: Path) -> bytes:
    # Let the provider execute its actual Read tool and return its result to the API.
    data = _response(model).decode()
    data = data.replace(
        '"type": "text", "text": ""',
        '"type": "tool_use", "id": "read_asset", "name": "Read", "input": {}',
    )
    old = {"type": "text_delta", "text": "fixture response"}
    new = {"type": "input_json_delta", "partial_json": json.dumps({"file_path": str(path)})}
    return (
        data.replace(json.dumps(old), json.dumps(new))
        .replace('"stop_reason": "end_turn"', '"stop_reason": "tool_use"')
        .encode()
    )


def _codex_response(path: Path, done: bool, shell: str) -> bytes:
    item = (
        {
            "type": "message",
            "id": "msg_done",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "fixture response"}],
        }
        if done
        else {
            "type": "function_call",
            "id": "fc_read",
            "call_id": "read_asset",
            "name": shell,
            "arguments": json.dumps(
                {
                    "cmd"
                    if shell == "exec_command"
                    else "command": f"cat {shlex.quote(str(path))}",
                    **({"max_output_tokens": 2000} if shell == "exec_command" else {}),
                }
            ),
        }
    )
    response = {"id": "resp_fixture", "status": "in_progress", "output": []}
    events = [
        ("response.created", {"response": dict(response)}),
        ("response.output_item.added", {"output_index": 0, "item": item}),
        ("response.output_item.done", {"output_index": 0, "item": item}),
    ]
    response.update(
        status="completed",
        output=[item],
        usage={
            "input_tokens": 20,
            "output_tokens": 5,
            "total_tokens": 25,
            "input_tokens_details": {"cached_tokens": 0},
        },
    )
    events.append(("response.completed", {"response": response}))
    return "".join(
        f"event: {kind}\ndata: {json.dumps({'type': kind, **value})}\n\n" for kind, value in events
    ).encode()


def _probe(
    lf: Path, provider: str, source: Path, asset_name: str, terminal: bool, flow: bool
) -> bool:
    requests = []
    with tempfile.TemporaryDirectory(prefix="lf-installed-skill-", dir="/tmp") as directory:
        root = Path(directory).resolve()
        work, home, bin_dir = root / "work", root / "home", root / "bin"
        for path in [work, home, bin_dir]:
            path.mkdir()
        dialect = "claude" if source.name == "internal-comms" else "codex"
        bundle = (
            work / (".claude/skills" if dialect == "claude" else ".agents/skills") / source.name
        )
        shutil.copytree(source, bundle)
        asset = bundle / asset_name
        source_bytes = (bundle / "SKILL.md").read_bytes()
        marker = "CONTEXT_MUST_REMAIN_USER_INPUT"
        context = work / "context.md"
        context.write_text(marker)
        argument = '"quoted argument"\nsecond line'
        assert not (work / ".lf").exists()

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, format: str, *args: object) -> None:
                pass

            def do_POST(self) -> None:
                body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                if self.path.split("?")[0] not in ("/v1/messages", "/v1/responses"):
                    self.send_error(404)
                    return
                requests.append(body)
                done = len(requests) % 2 == 0
                if provider == "claude":
                    response = (
                        _response(body["model"])
                        if done
                        else _claude_tool_response(body["model"], asset)
                    )
                else:
                    names = [tool.get("name") for tool in body.get("tools", [])]
                    response = _codex_response(
                        asset, done, "exec_command" if "exec_command" in names else "shell_command"
                    )
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Content-Length", str(len(response)))
                self.end_headers()
                self.wfile.write(response)

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        executable = str(Path(shutil.which(provider)).resolve())
        env = {key: os.environ[key] for key in ("PATH", "TMPDIR", "LANG") if key in os.environ}
        env.update(
            HOME=str(home),
            LF_HOME=str(root / "lf"),
            LF_BIN=str(lf),
            CLAUDE_CONFIG_DIR=str(home / ".claude"),
            CODEX_HOME=str(home / ".codex"),
            CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1",
            DISABLE_AUTOUPDATER="1",
            ANTHROPIC_BASE_URL=f"http://127.0.0.1:{server.server_port}",
        )
        Path(env["CODEX_HOME"]).mkdir()
        (Path(env["CODEX_HOME"]) / "config.toml").write_text(f"""model = "gpt-5.4"
model_provider = "fixture"
cli_auth_credentials_store = "file"
allow_login_shell = false
sandbox_mode = "danger-full-access"
approval_policy = "never"
[features]
shell_snapshot = false
[model_providers.fixture]
name = "Local fixture"
base_url = "http://127.0.0.1:{server.server_port}/v1"
wire_api = "responses"
requires_openai_auth = false
[analytics]
enabled = false
[feedback]
enabled = false
""")
        # --tui follows lf's native terminal launcher. Adapt only the final client
        # to print/exec for headless acceptance; this does not test terminal rendering.
        flags = (
            " -p --output-format stream-json --verbose"
            if terminal and provider == "claude"
            else " exec --skip-git-repo-check"
            if terminal
            else ""
        )
        prefix = "ANTHROPIC_API_KEY=local-fixture-not-a-credential " if provider == "claude" else ""
        wrapper = bin_dir / provider
        wrapper.write_text(f'#!/bin/sh\n{prefix}exec {shlex.quote(executable)}{flags} "$@"\n')
        wrapper.chmod(0o755)
        env["PATH"] = str(bin_dir) + os.pathsep + env["PATH"]
        target = [source.name, argument]
        if flow:
            flows = work / ".lf/flows"
            flows.mkdir(parents=True)
            (flows / "proof.yaml").write_text(f"- {source.name}\n")
            child = bin_dir / "lf"
            child.write_text(
                f"#!/bin/sh\nrm -f {shlex.quote(str(bundle / 'SKILL.md'))}\n"
                f'exec {shlex.quote(str(lf))} "$@"\n'
            )
            child.chmod(0o755)
            env["LF_BIN"] = str(child)
            target = ["flow", "proof", argument]
        try:
            start = time.monotonic()
            result = _run(
                [
                    str(lf),
                    "--tui" if terminal else "-b",
                    "--agent",
                    provider,
                    "--docs",
                    str(context),
                    *target,
                ],
                work,
                env,
            )
            seconds = time.monotonic() - start
            first = requests[0] if requests else {}
            items = first.get("messages", first.get("input", []))
            user_text = json.dumps(
                [item for item in items if item.get("role") == "user"], ensure_ascii=False
            )
            privileged = json.dumps(
                first.get("system", first.get("instructions", ""))
            ) + json.dumps([item for item in items if item.get("role") in ("system", "developer")])
            body = source_bytes.decode().split("---", 2)[-1].strip()
            last = requests[-1] if requests else {}
            tool_results = (
                [
                    item
                    for item in last.get("input", [])
                    if item.get("type") == "function_call_output"
                ]
                if provider == "codex"
                else [
                    block
                    for item in last.get("messages", [])
                    if isinstance(item.get("content"), list)
                    for block in item["content"]
                    if block.get("type") == "tool_result"
                ]
            )
            # A distinctive literal from the actual file must return in a tool result.
            asset_line = next(line for line in asset.read_text().splitlines() if len(line) > 40)
            checks = {
                "exit_zero": result.returncode == 0,
                "one_turn_and_asset_read": len(requests) == 2,
                "source_expanded": json.dumps(body[:100], ensure_ascii=False)[1:-1] in user_text,
                "exact_arguments": json.dumps(argument)[1:-1] in user_text,
                "context_user_only": marker in user_text and marker not in privileged,
                "provider_read_asset": json.dumps(asset_line)[1:-1] in json.dumps(tool_results),
                "source_retained": (not (bundle / "SKILL.md").exists())
                if flow
                else (bundle / "SKILL.md").read_bytes() == source_bytes,
                "no_lf_configuration": not (work / ".lf/config.yaml").exists(),
            }
            comparison = None
            if dialect == provider and not flow:
                requests.clear()
                if provider == "claude":
                    plain = [
                        str(wrapper),
                        *(
                            []
                            if terminal
                            else ["-p", "--output-format", "stream-json", "--verbose"]
                        ),
                        f"/{source.name} {argument}",
                    ]
                else:
                    plain = [
                        str(wrapper),
                        *([] if terminal else ["exec", "--skip-git-repo-check"]),
                        f"${source.name} {argument}",
                    ]
                started = time.monotonic()
                baseline = _run(plain, work, env)
                plain_seconds = time.monotonic() - started
                checks["same_model_request_count_as_plain"] = (
                    baseline.returncode == 0 and len(requests) == 2
                )
                comparison = {
                    "seconds": round(plain_seconds, 3),
                    "added_seconds": round(seconds - plain_seconds, 3),
                    "added_request_bytes": len(json.dumps(first)) - len(json.dumps(requests[0]))
                    if requests
                    else None,
                }
            print(
                json.dumps(
                    {
                        "source": source.name,
                        "provider": provider,
                        "surface": "terminal-command" if terminal else "headless",
                        "seconds": round(seconds, 3),
                        "request_bytes": len(json.dumps(first)),
                        "plain": comparison,
                        "source_sha256": hashlib.sha256(source_bytes).hexdigest(),
                        "checks": checks,
                    }
                ),
                flush=True,
            )
            if not all(checks.values()):
                print(result.stderr[-4000:], flush=True)
                print(
                    json.dumps(
                        {
                            "user_excerpt": user_text[:200],
                            "marker_in_user": marker in user_text,
                            "marker_in_privileged": marker in privileged,
                            "tools": [t.get("name") for t in first.get("tools", [])],
                            "tool_results": tool_results,
                        }
                    )[:1800],
                    flush=True,
                )
            return all(checks.values())
        finally:
            server.shutdown()
            server.server_close()
            thread.join()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lf", type=Path, default=Path("target/debug/lf"))
    parser.add_argument("--fixtures", type=Path, required=True)
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--provider", choices=("claude", "codex"), default="claude")
    parser.add_argument("--source", choices=tuple(SOURCES), default="internal-comms")
    parser.add_argument("--terminal", action="store_true")
    parser.add_argument("--flow", action="store_true")
    args = parser.parse_args()
    if args.fetch:
        _fetch(args.fixtures)
        return 0
    return (
        0
        if _probe(
            args.lf.resolve(),
            args.provider,
            args.fixtures / args.source,
            SOURCES[args.source][3],
            args.terminal,
            args.flow,
        )
        else 1
    )


if __name__ == "__main__":
    raise SystemExit(main())

import fcntl
import json
import os
from pathlib import Path
import pty
import select
import shlex
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

OUT = Path(__file__).resolve().parent
ROOT = Path(tempfile.mkdtemp(prefix="lf-claude-modes-")).resolve()
WORK = ROOT / "workspace"
WORK.mkdir()
requests = []
release = threading.Event()
release.set()


def response(model, text):
    events = [
        {
            "type": "message_start",
            "message": {
                "id": "msg_" + uuid.uuid4().hex,
                "type": "message",
                "role": "assistant",
                "model": model,
                "content": [],
                "stop_reason": None,
                "stop_sequence": None,
                "usage": {"input_tokens": 11, "output_tokens": 1},
            },
        },
        {"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}},
        {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": text}},
        {"type": "content_block_stop", "index": 0},
        {
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn", "stop_sequence": None},
            "usage": {"output_tokens": 7},
        },
        {"type": "message_stop"},
    ]
    return "".join(
        "event: " + e["type"] + "\ndata: " + json.dumps(e) + "\n\n" for e in events
    ).encode()


def tool_response(model, name, value):
    events = [
        {
            "type": "message_start",
            "message": {
                "id": "msg_" + uuid.uuid4().hex,
                "type": "message",
                "role": "assistant",
                "model": model,
                "content": [],
                "stop_reason": None,
                "stop_sequence": None,
                "usage": {"input_tokens": 11, "output_tokens": 1},
            },
        },
        {
            "type": "content_block_start",
            "index": 0,
            "content_block": {
                "type": "tool_use",
                "id": "toolu_" + uuid.uuid4().hex,
                "name": name,
                "input": {},
            },
        },
        {
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "input_json_delta", "partial_json": json.dumps(value)},
        },
        {"type": "content_block_stop", "index": 0},
        {
            "type": "message_delta",
            "delta": {"stop_reason": "tool_use", "stop_sequence": None},
            "usage": {"output_tokens": 7},
        },
        {"type": "message_stop"},
    ]
    return "".join(
        "event: " + e["type"] + "\ndata: " + json.dumps(e) + "\n\n" for e in events
    ).encode()


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        if self.path.split("?")[0] != "/v1/messages":
            self.send_error(404)
            return
        requests.append({"time": time.monotonic(), "body": body})
        release.wait(20)
        data = response(body["model"], "PROBE_REPLY_" + str(len(requests)))
        if mode.endswith("correction"):
            data = response(
                body["model"],
                "INVALID_DECISION" if len(requests) == 1 else '{"decision":"advance"}',
            )
        if mode.endswith("question") and len(requests) == 1:
            data = tool_response(
                body["model"],
                "AskUserQuestion",
                {
                    "questions": [
                        {
                            "question": "Which probe color?",
                            "header": "Color",
                            "options": [
                                {"label": "Blue", "description": "Choose blue."},
                                {"label": "Green", "description": "Choose green."},
                            ],
                            "multiSelect": False,
                        }
                    ]
                },
            )
        if mode.endswith("schema"):
            names = [t.get("name", "") for t in body.get("tools", [])]
            name = next((n for n in names if "structured" in n.lower()), None)
            if name:
                data = tool_response(body["model"], name, {"decision": "advance"})
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()

hook = ROOT / "hook.py"
hook.write_text(
    'import json,sys,time\nfrom pathlib import Path\nx=json.load(sys.stdin); x["observed_time"]=time.monotonic()\nwith Path(sys.argv[1]).open("a") as f: f.write(json.dumps(x)+"\\n")\n'
)


def environment(config):
    env = {k: os.environ[k] for k in ("PATH", "TMPDIR", "LANG") if k in os.environ}
    env.update(
        CLAUDE_CONFIG_DIR=str(config),
        ANTHROPIC_BASE_URL=f"http://127.0.0.1:{server.server_port}",
        ANTHROPIC_API_KEY="local-fixture-not-a-credential",
        CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1",
        DISABLE_AUTOUPDATER="1",
        TERM="xterm-256color",
    )
    return env


def setup(mode):
    config = ROOT / mode
    config.mkdir()
    (config / ".claude.json").write_text(
        json.dumps(
            {
                "hasCompletedOnboarding": True,
                "theme": "dark",
                "projects": {str(WORK): {"hasTrustDialogAccepted": True}},
            }
        )
    )
    hooks = ROOT / (mode + "-hooks.jsonl")
    cmd = shlex.join([sys.executable, str(hook), str(hooks)])
    if mode.endswith("correction"):
        correction = ROOT / "correction.py"
        correction.write_text(
            'import json,sys,time\nfrom pathlib import Path\nx=json.load(sys.stdin); x["observed_time"]=time.monotonic()\nwith Path(sys.argv[1]).open("a") as f: f.write(json.dumps(x)+"\\n")\nif x.get("hook_event_name")=="Stop" and x.get("last_assistant_message")=="INVALID_DECISION": print(json.dumps({"decision":"block","reason":"Return only a JSON object with decision equal to advance."}))\n'
        )
        cmd = shlex.join([sys.executable, str(correction), str(hooks)])
    settings = {
        "hooks": {
            event: [{"hooks": [{"type": "command", "command": cmd}]}]
            for event in [
                "SessionStart",
                "UserPromptSubmit",
                "Stop",
                "StopFailure",
                "PermissionRequest",
            ]
        }
    }
    settings_path = config / "settings.json"
    settings_path.write_text(json.dumps(settings))
    return config, hooks, settings_path


def read_hooks(path):
    return [json.loads(x) for x in path.read_text().splitlines()] if path.exists() else []


def drain(fd, duration):
    end = time.monotonic() + duration
    data = b""
    while time.monotonic() < end:
        if select.select([fd], [], [], min(0.05, end - time.monotonic()))[0]:
            try:
                part = os.read(fd, 65536)
            except OSError:
                break
            if not part:
                break
            data += part
    return data


claude = shutil.which("claude")
mode = sys.argv[1] if len(sys.argv) > 1 else "native"
is_stream = mode.startswith("stream")
config, hooks, settings = setup(mode)
base = [
    claude,
    "--setting-sources",
    "",
    "--settings",
    str(settings),
    "--strict-mcp-config",
    "--tools",
    "",
    "--model",
    "haiku",
    "--system-prompt",
    "Return the requested short probe marker. No tools.",
]
if mode.endswith("schema"):
    base += [
        "--json-schema",
        '{"type":"object","properties":{"decision":{"type":"string","enum":["advance"]}},"required":["decision"],"additionalProperties":false}',
    ]
if mode.endswith("question"):
    base[base.index("--tools") + 1] = "AskUserQuestion"
    base += ["--permission-mode", "manual"]
if is_stream:
    base += [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--replay-user-messages",
        "--verbose",
    ]
else:
    base += ["--name", "LOO415 disposable probe"]
start = time.monotonic()
fd = None
if is_stream:
    process = subprocess.Popen(
        base,
        cwd=WORK,
        env=environment(config),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    fd = process.stdout.fileno()
else:
    fd, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    process = subprocess.Popen(
        base,
        cwd=WORK,
        env=environment(config),
        stdin=slave,
        stdout=slave,
        stderr=slave,
        start_new_session=True,
    )
    os.close(slave)
raw = b""
summary = {"mode": mode, "root": str(ROOT), "pid": process.pid, "command": base, "requests": []}
try:
    raw += drain(fd, 4)
    if not is_stream:
        if b"Detected" in raw and b"API" in raw:
            os.write(fd, b"\x1b[A\r")
            raw += drain(fd, 4)
        (OUT / "native-startup.txt").write_bytes(raw)
        print(raw.decode(errors="replace")[-6000:], flush=True)

    def submit(text):
        if is_stream:
            value = {
                "type": "user",
                "message": {"role": "user", "content": text},
                "uuid": str(uuid.uuid4()),
            }
            process.stdin.write((json.dumps(value) + "\n").encode())
            process.stdin.flush()
            return value["uuid"]
        os.write(fd, text.encode() + b"\r")
        return None

    summary["first_id"] = submit("Reply with PROBE_ONE")
    raw += drain(fd, 7)
    print(
        "after-first",
        json.dumps(
            {"requests": len(requests), "hooks": read_hooks(hooks), "alive": process.poll() is None}
        ),
        flush=True,
    )
    summary["first_hooks"] = read_hooks(hooks)
    summary["rss_first"] = subprocess.run(
        ["ps", "-o", "rss=", "-p", str(process.pid)], capture_output=True, text=True
    ).stdout.strip()
    release.clear()
    if mode.endswith("question"):
        (OUT / (mode + "-pending.txt")).write_bytes(raw)
        os.write(fd, b"\r")
        summary["second_id"] = "answer-original-question"
    else:
        summary["second_id"] = submit("Reply with PROBE_TWO")
    raw += drain(fd, 3)
    summary["pending_alive"] = process.poll() is None
    summary["pending_requests"] = len(requests)
    # No viewer exists; this controller is the retaining PTY/pipe owner.
    release.set()
    raw += drain(fd, 4)
    summary["final_alive"] = process.poll() is None
    summary["hooks"] = read_hooks(hooks)
    summary["duration"] = time.monotonic() - start
    summary["requests"] = [{"time": r["time"] - start, "body": r["body"]} for r in requests]
    summary["events"] = []
    if is_stream:
        for line in raw.splitlines():
            try:
                summary["events"].append(json.loads(line))
            except ValueError:
                pass
    (OUT / (mode + "-output.txt")).write_bytes(raw)
    (OUT / (mode + "-result.json")).write_text(json.dumps(summary, indent=2))
    print(
        json.dumps(
            {k: v for k, v in summary.items() if k not in ["requests", "command", "events"]},
            indent=2,
        ),
        flush=True,
    )
finally:
    release.set()
    if fd is not None and not is_stream:
        os.close(fd)
        fd = None
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(3)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
    if fd is not None and not is_stream:
        os.close(fd)
    server.shutdown()

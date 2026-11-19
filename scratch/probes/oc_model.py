"""Does the OpenCode TUI answer with the `model` in OPENCODE_CONFIG_CONTENT,
for a new `--session` and for one reopened with a different configured model?"""
import json, os, subprocess, sys, tempfile, threading, time, uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests/e2e"))
from tui import _Tui

requests = []
class Provider(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        requests.append(body)
        def chunk(delta, finish):
            return {"id": "c", "object": "chat.completion.chunk", "created": 1, "model": body["model"],
                    "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]}
        parts = [chunk({"role": "assistant", "content": f"answered by {body['model']}"}, None), chunk({}, "stop")]
        data = "".join("data: " + json.dumps(v) + "\n\n" for v in parts) + "data: [DONE]\n\n"
        self.send_response(200); self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(data.encode()))); self.end_headers(); self.wfile.write(data.encode())

api = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
threading.Thread(target=api.serve_forever, daemon=True).start()
binary = sys.argv[1] if len(sys.argv) > 1 else "/opt/homebrew/bin/opencode"
base = Path(tempfile.mkdtemp(prefix="lf-ocm-", dir="/tmp"))
work = base / "work"; work.mkdir(); (base / "home").mkdir()
subprocess.run(["git", "init", "-q", str(work)], check=True)
model = lambda name: {"name": name, "tool_call": True, "limit": {"context": 128000, "output": 4096}}
def turn(configured: str | None, prompt: str, session: str) -> list[str]:
    settings = {"autoupdate": False, "share": "disabled", "enabled_providers": ["opencode"],
        "small_model": "opencode/small",
        "provider": {"opencode": {"npm": "@ai-sdk/openai-compatible", "options": {
            "baseURL": f"http://127.0.0.1:{api.server_port}/v1", "apiKey": "local-fixture-not-a-credential"},
            "models": {"neon": model("neon"), "carbon": model("carbon"), "small": model("small")}}},
        "permission": {"*": "allow"}}
    if configured:
        settings["model"] = configured
    env = dict(HOME=str(base / "home"), PATH="/opt/homebrew/bin:/usr/bin:/bin", PWD=str(work),
        TERM="xterm-256color", LANG="en_US.UTF-8", OPENCODE_DISABLE_DEFAULT_PLUGINS="true",
        OPENCODE_DISABLE_MODELS_FETCH="true", OPENCODE_API_KEY="local-fixture-not-a-credential",
        OPENCODE_CONFIG_CONTENT=json.dumps(settings))
    before = len(requests)
    tui = _Tui([binary, "--standalone", "--session", session, "--prompt", prompt], work, env)
    try:
        end = time.monotonic() + 40
        while time.monotonic() < end and not any(prompt in json.dumps(r) for r in requests[before:]):
            tui.drain(0.1)
            if tui.child.poll() is not None: break
        tui.drain(2.0)
        screen = "\n".join(tui.screen.display)
    finally:
        tui.close()
    asked = [r["model"] for r in requests[before:] if prompt in json.dumps(r)]
    return asked, [line.strip() for line in screen.splitlines() if "answered by" in line]

version = subprocess.check_output([binary, "--version"], text=True).strip()
first, second = ("ses_" + uuid.uuid4().hex for _ in range(2))
results = {
    "new session, config carbon": turn("opencode/carbon", "first prompt", first),
    "reopened, config neon": turn("opencode/neon", "second prompt", first),
    "reopened, no config model": turn(None, "third prompt", first),
    "another new session, same home, config neon": turn("opencode/neon", "fourth prompt", second),
}
state = base / "home/.local/state/opencode/model.json"
print(json.dumps({"version": version, **{k: v[0] for k, v in results.items()},
    "model.json": json.loads(state.read_text()) if state.exists() else None}, indent=1))

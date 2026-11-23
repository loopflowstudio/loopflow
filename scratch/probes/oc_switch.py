"""Can an OpenCode 2.2.1 plugin set a reopened session's model with
`session.switchModel`, called as the plugin loads (`setup`) or in the `prompt`
hook, and does a later turn keep it?"""
import json, subprocess, sys, tempfile, threading, time, uuid
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
base = Path(tempfile.mkdtemp(prefix="lf-ocs-", dir="/tmp"))
work = base / "work"; work.mkdir(); (base / "home").mkdir()
subprocess.run(["git", "init", "-q", str(work)], check=True)
log = base / "plugin.log"
plugin = base / "plugin"; plugin.mkdir()
(plugin / "package.json").write_text('{"name":"probe","type":"module","exports":{"./server":"./index.js"}}')
(plugin / "index.js").write_text("""
import { appendFileSync } from "node:fs";
const mode = process.env.PROBE_MODE, target = process.env.PROBE_SESSION, model = process.env.PROBE_MODEL;
const note = (line) => appendFileSync(process.env.PROBE_LOG, line + "\\n");
const ref = () => { const at = model.indexOf("/"); return { providerID: model.slice(0, at), id: model.slice(at + 1) }; };
export default { id: "probe", async setup({ session }) {
  if (mode === "setup") {
    try { await session.switchModel({ sessionID: target, model: ref() }); note("setup switched"); }
    catch (error) { note("setup failed: " + String(error?.message ?? error)); }
  }
  let done = false;
  await session.hook("prompt", async (input) => {
    if (mode !== "prompt" || done) return; done = true;
    try { await session.switchModel({ sessionID: input.sessionID, model: ref() }); note("prompt switched"); }
    catch (error) { note("prompt failed: " + String(error?.message ?? error)); }
  });
}};
""")
model = lambda name: {"name": name, "tool_call": True, "limit": {"context": 128000, "output": 4096}}
def turn(mode, switch_to, prompts, session):
    settings = {"autoupdate": False, "share": "disabled", "enabled_providers": ["opencode"],
        "small_model": "opencode/small", "model": "opencode/neon",
        "provider": {"opencode": {"npm": "@ai-sdk/openai-compatible", "options": {
            "baseURL": f"http://127.0.0.1:{api.server_port}/v1", "apiKey": "local-fixture-not-a-credential"},
            "models": {"neon": model("neon"), "carbon": model("carbon"), "small": model("small")}}},
        "permission": {"*": "allow"}, "plugins": [f"file://{plugin}"]}
    env = dict(HOME=str(base / "home"), PATH="/opt/homebrew/bin:/usr/bin:/bin", PWD=str(work),
        TERM="xterm-256color", LANG="en_US.UTF-8", OPENCODE_DISABLE_DEFAULT_PLUGINS="true",
        OPENCODE_DISABLE_MODELS_FETCH="true", OPENCODE_API_KEY="local-fixture-not-a-credential",
        OPENCODE_CONFIG_CONTENT=json.dumps(settings), PROBE_MODE=mode or "", PROBE_SESSION=session,
        PROBE_MODEL=switch_to or "", PROBE_LOG=str(log))
    before = len(requests)
    tui = _Tui([binary, "--standalone", "--session", session, "--prompt", prompts[0]], work, env)
    try:
        def wait_for(prompt):
            end = time.monotonic() + 40
            while time.monotonic() < end and not any(prompt in json.dumps(r) for r in requests[before:]):
                tui.drain(0.1)
                if tui.child.poll() is not None: return
            tui.drain(2.0)
        wait_for(prompts[0])
        for prompt in prompts[1:]:
            tui.enter(prompt)
            wait_for(prompt)
    finally:
        tui.close()
    return [(p, [r["model"] for r in requests[before:] if p in json.dumps(r)]) for p in prompts]

version = subprocess.check_output([binary, "--version"], text=True).strip()
first, second = ("ses_" + uuid.uuid4().hex for _ in range(2))
results = {
    "new session, config neon": turn(None, None, ["p1"], first),
    "reopened, switch to carbon at setup": turn("setup", "opencode/carbon", ["p2", "p3"], first),
    "reopened, no switch": turn(None, None, ["p4"], first),
    "other session: new, prompt-hook switch to carbon": turn("prompt", "opencode/carbon", ["p5"], second),
    "other session reopened, prompt-hook switch to neon": turn("prompt", "opencode/neon", ["p6"], second),
}
print(json.dumps({"version": version, **results, "plugin log": log.read_text().splitlines() if log.exists() else []}, indent=1))

import json, os, sys, tempfile, threading, time, subprocess
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
sys.path.insert(0, "/Users/jack/src/loopflow.loo-974-run-managed-tasks/tests/e2e")
from tui import _Tui
requests = []
class Provider(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        requests.append(body)
        parts = [{"id":"c","object":"chat.completion.chunk","created":1,"model":"proof","choices":[{"index":0,
            "delta":{"role":"assistant","content":f"fixture answer {len(requests)}"},"finish_reason":None}]},
            {"id":"c","object":"chat.completion.chunk","created":1,"model":"proof","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],
             "usage":{"prompt_tokens":31,"completion_tokens":7,"total_tokens":38}}]
        self.send_response(200); self.send_header("Content-Type","text/event-stream"); self.end_headers()
        for v in parts: self.wfile.write(("data: "+json.dumps(v)+"\n\n").encode())
        self.wfile.write(b"data: [DONE]\n\n")
EXT = r'''
import { existsSync, appendFileSync, unlinkSync } from "node:fs";
const log = (line) => appendFileSync(process.env.PROBE_LOG, line + "\n");
export default function (pi) {
  let idle = false;
  pi.on("agent_start", async () => { idle = false; log("agent_start"); });
  pi.on("agent_settled", async () => { idle = true; log("agent_settled"); });
  setInterval(async () => {
    if (!existsSync(process.env.PROBE_WAKE)) return;
    unlinkSync(process.env.PROBE_WAKE);
    try {
      if (process.env.PROBE_HOW === "user") await pi.sendUserMessage("WAKE_MARKER from lf");
      else await pi.sendMessage({ customType: "loopflow-steer", content: "WAKE_MARKER from lf", display: true }, { triggerTurn: true });
      log("woke via " + process.env.PROBE_HOW + " idle=" + idle);
    } catch (error) { log("wake failed: " + String(error?.message ?? error).slice(0, 300)); }
  }, 200);
}
'''
api = ThreadingHTTPServer(("127.0.0.1",0), Provider)
threading.Thread(target=api.serve_forever, daemon=True).start()
pi = "/Users/jack/.nvm/versions/node/v24.22.2/bin/pi"
out = Path(__file__).parent
for how in ["custom", "user"]:
    requests.clear()
    base = Path(tempfile.mkdtemp(prefix="lf-piw-", dir="/tmp"))
    work = base/"work"; work.mkdir(); agent = base/"agent"; agent.mkdir()
    subprocess.run(["git","init","-q",str(work)],check=True)
    (agent/"models.json").write_text(json.dumps({"providers":{"fixture":{"baseUrl":f"http://127.0.0.1:{api.server_port}/v1",
        "api":"openai-completions","apiKey":"fixture-not-a-secret","models":[{"id":"proof","contextWindow":200000}]}}}))
    (agent/"settings.json").write_text(json.dumps({"retry":{"enabled":False}}))
    (base/"ext.mjs").write_text(EXT)
    env = dict(HOME=str(base), PATH="/Users/jack/.nvm/versions/node/v24.22.2/bin:/usr/bin:/bin", TERM="xterm-256color",
        LANG="en_US.UTF-8", PI_CODING_AGENT_DIR=str(agent), PI_OFFLINE="1",
        PROBE_LOG=str(base/"ext.log"), PROBE_WAKE=str(base/"wake"), PROBE_HOW=how)
    tui = _Tui([pi,"--model","fixture/proof","--extension",str(base/"ext.mjs"),"First prompt typed by nobody"], work, env)
    result = {"version":subprocess.check_output([pi,"--version"],text=True,stderr=subprocess.STDOUT).strip(),"how":how}
    try:
        def wait(pred, secs):
            end=time.monotonic()+secs
            while time.monotonic()<end:
                tui.drain(0.1)
                if pred(): return True
                if tui.child.poll() is not None: return False
            return False
        result["first_turn"] = wait(lambda: len(requests)>=1, 30)
        tui.drain(3.0)
        before = len(requests)
        (base/"wake").write_text("x")
        result["turn_started_while_idle"] = wait(lambda: any("WAKE_MARKER" in json.dumps(r) for r in requests[before:]), 15)
        tui.drain(2.0)
        screen = "\n".join(tui.screen.display)
        result["marker_on_screen"] = "WAKE_MARKER" in screen
        result["second_answer_on_screen"] = "fixture answer 2" in screen
        if result["turn_started_while_idle"]:
            last = [r for r in requests if "WAKE_MARKER" in json.dumps(r)][-1]
            result["marker_role"] = [m["role"] for m in last["messages"] if "WAKE_MARKER" in json.dumps(m)]
        before = len(requests)
        tui.enter("typed after wake")
        result["typed_after_wake"] = wait(lambda: any("typed after wake" in json.dumps(r) for r in requests[before:]), 15)
        result["requests"] = len(requests)
        (out/f"pi-{how}.screen.txt").write_text("\n".join(tui.screen.display))
    finally:
        tui.close()
    result["ext_log"] = (base/"ext.log").read_text().splitlines()[:12] if (base/"ext.log").exists() else None
    print(json.dumps(result, indent=1), flush=True)

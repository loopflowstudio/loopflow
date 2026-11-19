import json, os, sys, tempfile, threading, time, subprocess, shutil
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
        def chunk(delta, finish):
            return {"id":"c","object":"chat.completion.chunk","created":1,"model":"glm-5.2",
                    "choices":[{"index":0,"delta":delta,"finish_reason":finish}]}
        parts = [chunk({"role":"assistant","content":f"fixture answer {len(requests)}"}, None), chunk({}, "stop"),
            {"id":"c","object":"chat.completion.chunk","created":1,"model":"glm-5.2","choices":[],
             "usage":{"prompt_tokens":31,"completion_tokens":7,"total_tokens":38}}]
        data = "".join("data: "+json.dumps(v)+"\n\n" for v in parts)+"data: [DONE]\n\n"
        self.send_response(200); self.send_header("Content-Type","text/event-stream")
        self.send_header("Content-Length",str(len(data.encode()))); self.end_headers(); self.wfile.write(data.encode())

PLUGIN = r'''
import { existsSync, appendFileSync, unlinkSync } from "node:fs";
const log = (line) => appendFileSync(process.env.PROBE_LOG, line + "\n");
export default {
  id: "wake-probe",
  async setup(api) {
    const { session, event } = api;
    log("setup keys " + Object.keys(api).join(",") + " | session " + Object.keys(session).join(","));
    let owner;
    await session.hook("prompt", (input) => { owner ??= input.sessionID; log("prompt " + input.sessionID); });
    const controller = new AbortController();
    (async () => {
      for await (const item of event.subscribe({ signal: controller.signal })) {
        if (item.type.startsWith("session.execution")) log("event " + item.type);
      }
    })().catch((e) => log("events " + e.message));
    const timer = setInterval(async () => {
      const wake = process.env.PROBE_WAKE;
      if (!owner || !existsSync(wake)) return;
      unlinkSync(wake);
      const how = process.env.PROBE_HOW;
      try {
        const result = how === "prompt"
          ? await session.prompt({ sessionID: owner, text: "WAKE_MARKER from lf" })
          : await session.synthetic({ sessionID: owner, text: "WAKE_MARKER from lf" });
        log("woke via " + how + " -> " + JSON.stringify(result ?? null).slice(0, 200));
      } catch (error) { log("wake failed via " + how + ": " + String(error?.message ?? error).slice(0, 300)); }
    }, 200);
    return async () => { clearInterval(timer); controller.abort(); };
  },
};
'''
api = ThreadingHTTPServer(("127.0.0.1",0), Provider)
threading.Thread(target=api.serve_forever, daemon=True).start()
out = Path(__file__).parent
for binary in ["/opt/homebrew/bin/opencode", str(Path.home()/".opencode/bin/opencode")]:
  for how in ["synthetic", "prompt"]:
    requests.clear()
    base = Path(tempfile.mkdtemp(prefix="lf-ocw-", dir="/tmp"))
    work = base/"work"; work.mkdir(); (base/"home").mkdir()
    subprocess.run(["git","init","-q",str(work)],check=True)
    plugin = base/"plugin"; plugin.mkdir()
    (plugin/"index.js").write_text(PLUGIN)
    (plugin/"package.json").write_text('{"name":"wake-probe","type":"module","exports":{"./server":"./index.js"}}')
    version = subprocess.check_output([binary,"--version"],text=True).strip()
    prov = "fixture" if version.endswith("2.0.32") else "opencode"
    settings = {"model":f"{prov}/glm-5.2","small_model":f"{prov}/glm-5.2","autoupdate":False,"share":"disabled",
        "enabled_providers":[prov],"plugins":[f"file://{plugin}"],
        "provider":{prov:{"npm":"@ai-sdk/openai-compatible","options":{"baseURL":f"http://127.0.0.1:{api.server_port}/v1",
        "apiKey":"local-fixture-not-a-credential"},"models":{"glm-5.2":{"name":"fixture","tool_call":True,
        "limit":{"context":128000,"output":4096}}}}},"permission":{"*":"allow"}}
    env = dict(HOME=str(base/"home"), PATH="/opt/homebrew/bin:/usr/bin:/bin", PWD=str(work), TERM="xterm-256color",
        LANG="en_US.UTF-8", OPENCODE_DISABLE_DEFAULT_PLUGINS="true", OPENCODE_DISABLE_MODELS_FETCH="true",
        OPENCODE_API_KEY="local-fixture-not-a-credential", OPENCODE_CONFIG_CONTENT=json.dumps(settings),
        PROBE_LOG=str(base/"plugin.log"), PROBE_WAKE=str(base/"wake"), PROBE_HOW=how)
    tui = _Tui([binary,"--standalone","--prompt","First prompt typed by nobody"], work, env)
    result = {"version":version,"how":how}
    try:
        def wait(pred, secs):
            end=time.monotonic()+secs
            while time.monotonic()<end:
                tui.drain(0.1)
                if pred(): return True
                if tui.child.poll() is not None: return False
            return False
        result["first_turn"] = wait(lambda: len(requests)>=1, 30)
        if not result["first_turn"]:
            os.write(tui.master, b"\r"); result["first_turn_after_enter"] = wait(lambda: len(requests)>=1, 15)
        tui.drain(3.0)   # let it settle idle
        before = len(requests)
        (base/"wake").write_text("x")
        result["turn_started_while_idle"] = wait(lambda: any("WAKE_MARKER" in json.dumps(r) for r in requests[before:]), 15)
        tui.drain(2.0)
        screen = "\n".join(tui.screen.display)
        result["marker_on_screen"] = "WAKE_MARKER" in screen
        result["second_answer_on_screen"] = f"fixture answer {len(requests)}" in screen or "fixture answer 2" in screen
        result["requests"] = len(requests)
        if result["turn_started_while_idle"]:
            last = [r for r in requests if "WAKE_MARKER" in json.dumps(r)][-1]
            result["marker_role"] = [m["role"] for m in last["messages"] if "WAKE_MARKER" in json.dumps(m)]
        # typed input still works after a woken turn
        before = len(requests)
        tui.enter("typed after wake")
        result["typed_after_wake"] = wait(lambda: any("typed after wake" in json.dumps(r) for r in requests[before:]), 15)
        (out/f"oc-{version.split()[-1]}-{how}.screen.txt").write_text("\n".join(tui.screen.display))
    finally:
        tui.close()
    result["plugin_log"] = (base/"plugin.log").read_text().splitlines()[:12] if (base/"plugin.log").exists() else None
    print(json.dumps(result, indent=1), flush=True)

# fakellm.py <port> <log>: OpenAI-compatible chat endpoint that drives a scripted turn.
# Each turn: QA_TOOLS bash calls (each sleeps QA_SLEEP s), then a reply naming every QA_STEER* it saw.
import json, os, re, sys, time, uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT, LOG = int(sys.argv[1]), sys.argv[2]
TOOLS = int(os.environ.get("QA_TOOLS", "3"))
SLEEP = os.environ.get("QA_SLEEP", "4")

def log(entry):
    with open(LOG, "a") as f:
        f.write(json.dumps(entry) + "\n")

def plan(body):
    msgs = body.get("messages", [])
    last_user = max((i for i, m in enumerate(msgs) if m.get("role") == "user"), default=-1)
    done = sum(1 for m in msgs[last_user + 1:] if m.get("role") == "tool")
    text = json.dumps(msgs)
    steers = sorted(set(re.findall(r"QA_STEER\w*", text)))
    tool = next((t["function"]["name"] for t in body.get("tools", []) if t["function"]["name"].lower() in ("bash", "shell")), None)
    log({"t": time.time(), "messages": len(msgs), "tool_results": done, "steers": steers,
         "lf_block": "Current reference context, not a new request" in text, "last_user": json.dumps(msgs[last_user].get("content"))[:400] if last_user >= 0 else None, "system_has_steer": bool(re.search(r"QA_STEER", json.dumps([m for m in msgs if m.get("role")=="system"]))), "tool_steer": bool(re.search(r"QA_STEER", json.dumps([m for m in msgs[last_user+1:]]))), "tools": [t["function"]["name"] for t in body.get("tools", [])][:40]})
    ctl = json.load(open(LOG + ".ctl")) if os.path.exists(LOG + ".ctl") else {}
    tools, cmd = ctl.get("tools", TOOLS), ctl.get("cmd", f"sleep {SLEEP}")
    if tool and done < tools:
        return {"tool": tool, "args": {"command": f"{cmd}; echo qa-step-{done + 1}", "description": f"qa step {done + 1}"}}
    return {"text": "QA_DONE steers=" + (",".join(steers) or "none")}

class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        out = json.dumps({"object": "list", "data": [{"id": "qa-model", "object": "model"}]}).encode()
        self.send_response(200); self.send_header("content-type", "application/json"); self.send_header("content-length", str(len(out))); self.end_headers(); self.wfile.write(out)
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("content-length", 0))) or b"{}")
        p = plan(body)
        cid = "chatcmpl-" + uuid.uuid4().hex[:8]
        if "tool" in p:
            delta = {"role": "assistant", "content": None, "tool_calls": [{"index": 0, "id": "call_" + uuid.uuid4().hex[:8], "type": "function", "function": {"name": p["tool"], "arguments": json.dumps(p["args"])}}]}
            finish = "tool_calls"
        else:
            delta = {"role": "assistant", "content": p["text"]}
            finish = "stop"
        usage = {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        if body.get("stream"):
            self.send_response(200); self.send_header("content-type", "text/event-stream"); self.end_headers()
            for chunk in ({"choices": [{"index": 0, "delta": delta, "finish_reason": None}]},
                          {"choices": [{"index": 0, "delta": {}, "finish_reason": finish}], "usage": usage}):
                chunk.update({"id": cid, "object": "chat.completion.chunk", "created": int(time.time()), "model": "qa-model"})
                self.wfile.write(b"data: " + json.dumps(chunk).encode() + b"\n\n"); self.wfile.flush()
            self.wfile.write(b"data: [DONE]\n\n"); self.wfile.flush()
        else:
            msg = dict(delta)
            out = json.dumps({"id": cid, "object": "chat.completion", "created": int(time.time()), "model": "qa-model",
                              "choices": [{"index": 0, "message": msg, "finish_reason": finish}], "usage": usage}).encode()
            self.send_response(200); self.send_header("content-type", "application/json"); self.send_header("content-length", str(len(out))); self.end_headers(); self.wfile.write(out)

ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()

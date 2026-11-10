import html
import json
from pathlib import Path

rows = [json.loads(line) for line in Path("scratch/qa-transcript.jsonl").read_text().splitlines()]
passed = [row["value"] for row in rows if row["kind"] == "passed"]
assert len(passed) == 10
assert rows[-1]["kind"] == "cleanup_owners"
frames = [row["value"] for row in rows if row["kind"] == "receive"]
swift = next(row["value"]["stdout"] for row in rows if row["kind"] == "command" and "decode" in row["value"]["argv"])
base = next(row["value"]["private_directory"] for row in rows if row["kind"] == "cleanup")
revision = next(row["value"]["revision"] for row in rows if row["kind"] == "environment")
samples = [
    ("Live comment", next(frame for frame in frames if frame["type"] == "event" and frame["event"]["kind"] == "task.steer")),
    ("Real Graph exit", next(frame for frame in frames if frame["type"] == "event" and frame["event"]["kind"] == "graph.step_finished")),
    ("Empty comment", next(frame for frame in frames if frame["type"] == "error" and frame.get("id") == "empty" and frame["code"] != "busy")),
]
refusal = next((frame for frame in frames if frame["type"] == "error" and frame.get("code") == "busy"), None)
if refusal is not None:
    samples.append(("Admission refusal", refusal))
page = """<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>LOO-802 · Typed events QA</title>
<style>body{margin:0;background:#f6f7f2;color:#21332f;font:17px/1.6 system-ui,sans-serif}main{max-width:920px;margin:48px auto;padding:0 24px 50px}h1{font-size:40px;line-height:1.15}h2{margin-top:34px;font-size:23px}.eyebrow{font-size:13px;letter-spacing:2px;color:#52645d}.result{border-left:4px solid #397760;padding:14px 20px;background:#e8eee6}.limit{border-left:4px solid #a06b35;padding:14px 20px;background:#f4ebdc}a{color:#235c49}li{margin:10px 0}code,pre{font-family:ui-monospace,monospace;font-size:13px}pre{overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere;background:#edf0e9;padding:16px}details{border-top:1px solid #ced8d1;padding:14px 0}summary{cursor:pointer;font-weight:600}footer{margin-top:35px;border-top:1px solid #ced8d1;padding-top:15px;font-size:14px}</style>
<main><div class="eyebrow">LOO-802 / 9 NOVEMBER 2026 / HEADLESS WALKTHROUGH</div><h1>Read the state.<br>React to what happened.</h1>
<p class="result"><strong>No typed-event defect found in the exercised flows.</strong> The branch-built server delivered real steer and Graph completion events through its Unix socket. Swift production protocol types encoded live requests and decoded replies.</p>
<h2>What ran</h2><p>Built <code>cargo build -p loopflow --bin lf</code> and <code>swift build --package-path swift --target Loopflow -Xswiftc -gnone</code>. Used a disposable Git repository, bare local origin, local identity, private HOME and LF_HOME, cleared inherited LF_* variables, and closed CLI stdin. No mock endpoint or SQL seed was used.</p>
<p>The installed signed CLI prepared a Session; the branch opened the same private Machine and read it. Ordinary CLI commands created Tasks, wrote a multiline Unicode comment, and ran a two-step <code>cmd: help</code> Graph. Socket watches covered full and selected Task readings, plans, and a saved dashboard query. The walkthrough wrote while disconnected, sent eight writes together, retried admission refusals, rejected empty input, and restarted the server.</p>
<h2>Observed results</h2><ol>"""
page += "".join("<li>" + html.escape(result) + ".</li>" for result in passed)
page += "</ol><h2>Wire transcripts</h2><p>Event comments matched the CLI’s <code>comments.comments</code> entries exactly, including provenance markers. Overlapping watches shared occurrence identity and one connection sequence.</p>"
for title, frame in samples:
    page += "<details><summary>" + html.escape(title) + "</summary><pre>" + html.escape(json.dumps(frame, indent=2, ensure_ascii=False)) + "</pre></details>"
page += "<pre>" + html.escape(swift) + "</pre>"
page += """<p><a href="qa-transcript.jsonl">Complete command and socket transcript</a> · <a href="qa_events.py">Walkthrough driver</a> · <a href="qa_swift_probe.swift">Swift probe</a></p>
<h2>What QA uncovered</h2><p><strong>Task checkout starts a background primary conversation when a provider CLI is installed.</strong> Several early walkthroughs left terminal owners and Codex observers after their command-only Graphs completed. Inspection traced this to existing <code>ops/human_session/primary.rs</code> behavior, outside the typed-event change. All processes belonging to those disposable checkouts were stopped. The driver now isolates CODEX_HOME and CLAUDE_CONFIG_DIR and stops private terminal owners and descendants. The complete walkthrough was rerun with that setup.</p>
<p>No production repair was needed. The QA skill now records that launch behavior, the original comment payload, the thread-reading command, and saved Task queries. Other probe assumptions were corrected: comments retain provenance markers; <code>lf comment</code> writes; the default dashboard can omit unstarted Tasks; and admission refusals require retrying before checking write results.</p>
<h2>What remains unchecked</h2><div class="limit"><p><strong>Live task.shipped, including a ship leaving a plan.</strong> This requires an actual recorded GitHub merge. Publishing and landing are outside this QA step; no synthetic merge was substituted. Existing focused tests cover the store-level scenario, but that is not a live walkthrough.</p>
<p><strong>Desktop presentation and automatic client recovery.</strong> This was a headless protocol walkthrough, not a mounted window session or an end-to-end ServerClient recovery probe. It does not establish board animation, thread presentation, focus, or interaction. New UI consumers are outside this Task.</p>
<p><strong>Fault injection and maximum load.</strong> Unknown event kinds, sequence gaps, 256-event overflow, and slow-reader timeout were not produced by the live server. Prior focused tests recorded in the design cover client gap and unknown-kind handling. Affected-suite gate and CI remain later steps.</p></div>
<h2>End state</h2><p>CLI and socket state agreed. All eight burst comments remained after rejecting empty input. Foreground servers were stopped and reaped; private terminal owners and descendants were stopped separately. No unresolved typed-event defect was found in the exercised scenarios.</p><p>Private state retained for inspection: <code>""" + html.escape(base) + """</code>. The compiled Swift probe was removed after use; source and transcripts remain.</p><footer>Build under review: """ + html.escape(revision[:9]) + """. This step did not publish, land, or move the Task’s Graph.</footer></main></html>"""
Path("scratch/qa.html").write_text(page)
print(swift.strip())
print("10 live scenarios recorded in scratch/qa.html")

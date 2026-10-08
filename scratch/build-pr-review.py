"""Build the local PR walkthrough from pinned source and captured demo output."""

import html
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
HEAD = "0cd8e7f14a6bffecd9323541e7689570316b892a"
BASE = "35e759aaf4e79e7dc8eef2e8d30048f10172b45a"
REMOTE = "https://github.com/loopflowstudio/loopflow"


def _source(path: str, revision: str = HEAD) -> list[str]:
    return subprocess.check_output(["git", "show", f"{revision}:{path}"], cwd=ROOT, text=True).splitlines()


def _excerpt(path: str, symbol: str, ranges: list[tuple[int, int]], revision: str = HEAD) -> str:
    lines = _source(path, revision)
    sections = ["\n".join(lines[start - 1:end]) for start, end in ranges]
    text = "\n\n// … intervening source omitted …\n\n".join(sections)
    start, end = ranges[0][0], ranges[-1][1]
    label = ", ".join(f"{first}–{last}" for first, last in ranges)
    url = f"{REMOTE}/blob/{revision}/{path}#L{start}-L{end}"
    return (f'<figure class="source"><figcaption><a href="{url}">{html.escape(path)}</a>'
            f' · {html.escape(symbol)}<br>{revision[:9]} · lines {label}</figcaption>'
            f'<pre tabindex="0" data-source="{path}" data-revision="{revision}" data-ranges="{label}">'
            f'<code>{html.escape(text)}</code></pre></figure>')


def _terminal(text: str, caption: str) -> str:
    return f'<figure class="terminal"><figcaption>{caption}</figcaption><pre tabindex="0"><code>{html.escape(text)}</code></pre></figure>'


def main() -> None:
    raw = (ROOT / "scratch/remote-task-demo-output.txt").read_text()
    transcript = re.sub(r"\x1b\[[0-9;]*m", "", raw)
    observed = json.loads(re.search(r"AFTER TWO LAUNCHES: (.+)", transcript).group(1))
    commands = re.findall(r"^\$ .+", transcript, re.M)
    cold = "\n".join(re.findall(r"COLD MACHINE [^\n]+", transcript))
    error = re.search(r"^Error: Task INF-123.+", transcript, re.M).group(0)
    unpushed = re.search(r"SOURCE UNPUSHED \(transport operation\): (.+)", transcript).group(1)
    warning = re.search(r"^lf versions differ:.+", transcript, re.M).group(0)
    task = "rust/loopflow/src/ops/task.rs"
    transport = "rust/loopflow/src/ops/task/remote.rs"
    css = """
:root{color-scheme:light;--ink:#242827;--muted:#535e59;--line:#d8ded9;--paper:#fafbf8;--code:#eef1ec;--link:#175b47}
*{box-sizing:border-box}html{scroll-behavior:auto}body{margin:0;background:var(--paper);color:var(--ink);font:17px/1.65 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}main{max-width:1140px;margin:auto;padding:56px 46px 90px}a{color:var(--link);text-underline-offset:3px}a:hover{text-decoration-thickness:2px}a:focus-visible,summary:focus-visible,pre:focus-visible{outline:3px solid #a56026;outline-offset:5px}.skip{position:absolute;left:10px;top:-100px}.skip:focus{top:10px}nav{display:flex;gap:24px;flex-wrap:wrap;font-size:14px;margin:0 0 44px}h1{font:600 clamp(30px,5vw,46px)/1.15 Georgia,serif;letter-spacing:-.7px;margin:14px 0 22px}h2{font:500 30px/1.25 Georgia,serif;margin:0 0 18px}h3{font-size:18px;margin:28px 0 10px}.kicker,.meta,figcaption{font-size:13px;color:var(--muted)}.kicker{letter-spacing:.1em;text-transform:uppercase}.lead{font-size:20px;max-width:850px}p{max-width:900px;margin:12px 0 20px}section{padding-top:42px;margin-top:38px;border-top:1px solid var(--line);scroll-margin-top:20px}.map{width:100%;border-collapse:collapse;margin-top:32px;font-size:15px}.map th{text-align:left;font-size:12px;text-transform:uppercase;letter-spacing:.05em;color:var(--muted)}.map th,.map td{padding:15px 22px 15px 0;border-bottom:1px solid var(--line);vertical-align:top}.map th:first-child{width:22%}.map th:nth-child(2){width:35%}figure{margin:26px 0 32px;min-width:0}figcaption{margin-bottom:8px;overflow-wrap:anywhere}pre{margin:0;background:var(--code);padding:23px 26px;border-left:2px solid #94a79b;font:14px/1.65 ui-monospace,SFMono-Regular,Consolas,monospace;white-space:pre-wrap;overflow-wrap:anywhere;tab-size:4}code{font-family:ui-monospace,SFMono-Regular,Consolas,monospace;font-size:.88em}pre code{font-size:inherit}.terminal pre{background:#e8eee9;border-left-color:#24624b}.note{border-left:2px solid #a56026;padding-left:18px;margin:25px 0;color:#3f4943}.evidence{font-size:15px;color:var(--muted)}details{margin:24px 0;border-top:1px solid var(--line);border-bottom:1px solid var(--line);padding:15px 0}summary{cursor:pointer;font-size:15px;color:var(--link)}li{margin:9px 0}footer{margin-top:55px;font-size:13px;color:var(--muted)}@media(max-width:650px){main{padding:30px 20px 60px}nav{gap:12px 20px;margin-bottom:30px}body{font-size:16px}h2{font-size:27px}.lead{font-size:18px}pre{font-size:12px;padding:16px 14px;line-height:1.65}.map,.map tbody,.map tr,.map td{display:block}.map thead{display:none}.map tr{padding:14px 0;border-bottom:1px solid var(--line)}.map td{border:0;padding:5px 0}.map td::before{content:attr(data-label);display:block;font-size:11px;font-weight:650;text-transform:uppercase;letter-spacing:.04em;color:var(--muted)}section{padding-top:30px;margin-top:30px}}@media print{body{background:white;font-size:10pt}main{max-width:none;padding:0}nav,.skip{display:none}section{break-before:auto}pre{font-size:8pt;background:#f5f5f5!important}h2,h3,figcaption{break-after:avoid}a{color:inherit}details{display:block}details>*{display:block!important}footer{break-before:avoid}}
"""
    opening = f"""<a class="skip" href="#send">Skip to the walkthrough</a><main>
<nav aria-label="Review sections"><a href="#send">Send pushed work</a><a href="#adopt">Adopt and repeat</a><a href="#identity">Identity and planning</a><a href="#preserve">Keep local work</a><a href="#evidence">Evidence</a></nav>
<header><p class="note"><strong>Earlier adoption review.</strong> Jack Heart subsequently selected host callbacks. This page records the previous implementation; see the <a href="work-on-another-machine-name.md">revised design</a>.</p><div class="kicker">LOO-412 · PR #1491 · October 8, 2026</div><h1>A Task, on a machine that has never seen it</h1>
<p class="lead">Select the machine and issue. The target picks up the pushed branch, creates its local Task once, and runs the skill in that checkout.</p>
<p class="meta"><a href="{REMOTE}/pull/1491">Published PR #1491</a> · main <code>{BASE[:9]}</code> → <code>{HEAD[:9]}</code><br>Published head and local HEAD match. Only the local demo and review artifacts are unpublished.</p>
<p><strong>Built and exercised here.</strong> Five captured scenarios passed with this worktree’s <code>target/debug/lf</code>. The public selector ran twice against a fresh target store. Git and both CLI processes are real; SSH, GitHub and the agent provider are simulated. No installed store was used.</p>
<p class="evidence">Scope: Task adoption and pushed-code selection. The global machine selector and saved connection already exist in main from LOO-411. Credentials, disconnection survival and remote Desktop restoration are outside this PR.</p></header>
<table class="map"><thead><tr><th>When you…</th><th>What happens</th><th>Where the code controls this</th></tr></thead><tbody>
<tr><td data-label="When you…"><a href="#send">Run a skill on another machine</a></td><td data-label="What happens">The source checks that its branch and commit are pushed before connecting.</td><td data-label="Where the code controls this"><code>TaskSource::resolve</code> builds the branch, commit and planning record sent with this invocation.</td></tr>
<tr><td data-label="When you…"><a href="#adopt">Name an absent Task, then repeat</a></td><td data-label="What happens">The first launch prepares the checkout. The second returns the stored Task and reuses it.</td><td data-label="Where the code controls this"><code>resolve_task</code> replaces the local-only lookup and calls existing Task preparation on a miss.</td></tr>
<tr><td data-label="When you…"><a href="#identity">Start the same issue in two fresh stores</a></td><td data-label="What happens">Both new Tasks receive the same ID; an existing Task keeps its ID and local execution history.</td><td data-label="Where the code controls this"><code>TaskId::from_issue</code> derives new identity; <code>create_prepared_task</code> owns local registration and Project selection.</td></tr>
<tr><td data-label="When you…"><a href="#preserve">Return to a checkout behind the requested commit</a></td><td data-label="What happens">The launch stops with a branch, commit and <code>lf sync</code> instruction. Existing files and HEAD survive.</td><td data-label="Where the code controls this"><code>require_checkout</code> checks the retained branch and ancestry without resetting it.</td></tr>
</tbody></table>
"""
    send = '<section id="send"><h2>1. Send the code that is already pushed</h2><p>The source names the issue across the connection, even when its local Task has a legacy ID. Only the branch, required commit and planning observation travel; target paths, Workflow and Session state stay local.</p>'
    send += _terminal(commands[1] + '\n\nexit: 0\nstdout: empty\nstderr ends with: ok\n[launch diagnostics omitted; retained in the full capture]', 'Captured public command · worktree CLI at ' + HEAD[:9] + ' · simulated connection “fixture”')
    send += _excerpt(transport, 'TaskSource and TaskSource::resolve', [(14, 50)])
    send += '<p class="note"><code>PmTaskRecord</code> is an existing planning observation: issue, optional Project and observation time. This PR reuses it rather than copying Task identity or execution authority. Pushed-code validation precedes planning refresh, so a planning outage cannot hide missing code.</p>'
    send += _excerpt(transport, 'require_pushed_code', [(133, 149)])
    send += '<p class="evidence">The fetch updates all origin branch refs before ancestry decisions, including branches without PRs. Missing code returns an error; neither this function nor the transport commits or pushes it.</p></section>'
    adopt = '<section id="adopt"><h2>2. Adopt once; run in the same checkout twice</h2><p>The target clone existed before the implementation branch. The first command began with zero Tasks. The second selected the same machine by its ID.</p>'
    adopt += _terminal(commands[2] + '\n\nexit: 0\nstdout: empty\nstderr ends with: ok\n[launch diagnostics omitted]\n\nObserved by the demo harness after both commands:\n' + json.dumps(observed, indent=2), 'Captured repeat command and store/filesystem observations · not invented CLI output')
    adopt += _excerpt(task, 'resolve_task', [(768, 797)])
    adopt += '<p>Previously, global <code>--task</code> returned “is not registered” on a miss. It now uses this resolver. The existing-Task branch preserves identity; a missing local Task ID asks for the issue name needed for adoption.</p>'
    adopt += _excerpt(task, 'prepare_task — cold adoption', [(894, 945)])
    adopt += '<p class="note">The transported branch overrides the planning record’s branch name for placement. After preparation, the required commit is checked against the actual checkout. The CLI then enters that directory and clears <code>LF_TASK_SOURCE</code> before launching the skill.</p>'
    adopt += '<details><summary>Entry dispatch and the local Task binding</summary>'
    adopt += _excerpt('rust/loopflow/src/ops/run.rs', 'resolve_work_selection — global Task selection', [(123, 135)])
    adopt += _excerpt('rust/loopflow/src/bin/lf.rs', 'dispatch — bind directory and consume invocation input', [(1525, 1543)])
    adopt += '</details></section>'
    identity = '<section id="identity"><h2>3. Share issue identity; keep local planning authority</h2><p>Two other fresh stores adopted the same issue through the built CLI. Their new Task IDs, checked-out commit and retained planning timestamp matched.</p>'
    identity += _terminal(cold, 'Captured harness observations · separate cold-adoption scenario; Task source supplied directly')
    identity += _excerpt('rust/loopflow/src/durable.rs', 'TaskId::from_issue', [(74, 81)])
    identity += '<p>The caller uses this constructor only for new Tasks. Existing Tasks are returned by the shared store query. A cold machine can select the issue’s active Project only when its Wave has no selection and no pending rotation.</p>'
    identity += _excerpt(task, 'create_prepared_task — Project selection, existing identity, local Task and storage', [(1227, 1304), (1330, 1340)])
    identity += '<p class="note"><strong>Remaining proof:</strong> the capture preserves an origin legacy ID and reuses a new target Task. It does not start with two different pre-existing IDs, populated target Workflow/Session history, an existing Project selection, or a pending rotation. These remain gate checks, not observed preservation claims.</p>'
    identity += '<details><summary>How transported planning respects target facts</summary><p>Planning is copied only when unavailable, after checking repository Team and Wave ownership. The guard is acquired and availability checked again before the write. Removal preservation passed; the separate invalidation case is not captured.</p>'
    identity += _excerpt(transport, 'TaskSource::accept_planning', [(56, 107)])
    identity += '</details></section>'
    preserve = '<section id="preserve"><h2>4. Keep work when the target falls behind</h2><p>The demonstration added local notes at the target, then pushed a newer source commit. Repeating the same command stopped before another skill run.</p>'
    failure = _terminal(commands[3] + '\n\nexit: 1\n' + error + '\n\nHarness observation:\n' + re.search(r'^PRESERVED AFTER REJECTION:.+', transcript, re.M).group(0), 'Captured failure and independent readback · version warning omitted here')
    preserve += failure
    preserve += _excerpt(transport, 'TaskSource::require_checkout', [(113, 121)])
    preserve += '<p>The caller explicitly syncs in the named checkout before continuing. That recovery was not executed in this demonstration. The comparison accepts a target ahead of the required commit on the same branch; exact HEAD equality is not required.</p>'
    preserve += _terminal(unpushed, 'Captured source-side rejection · direct transport operation, not the public CLI')
    preserve += '<p class="evidence">Separate passing scenarios retain uncommitted/unpushed source files and HEAD, refuse a missing remote branch or commit without creating a Task, and retain target removal evidence.</p></section>'
    evidence = '<section id="evidence"><h2>What this review establishes</h2><p>Jack Heart required a CLI built from this worktree. The earlier note treating installed 0.13.9 as the demo prerequisite is superseded. This walkthrough uses the branch build; no experiential approval or merge decision is recorded.</p>'
    evidence += '<ul><li><strong>Newly run:</strong> <code>cargo build -p loopflow --bin lf</code>; all five instrumented remote-Task scenarios under external-network isolation.</li><li><strong>Observed boundary:</strong> source CLI → machine selector → generated transport preamble → independent target CLI → SQLite Task and Git checkout → stub skill reading pushed code. The transport adapter executes the actual preamble locally.</li><li><strong>Not established:</strong> real SSH/authentication, installed migration or provider behavior, all preserved-history cases above, or a full affected gate.</li></ul>'
    evidence += '<h3>One visible rough edge, inherited from main</h3><p>The source and target used the same compiled binary, yet every connection printed this warning:</p>'
    evidence += _terminal(warning, 'Captured warning · unchanged machine version comparison')
    evidence += '<p>The comparison uses package version <code>0.13.9</code> locally while the remote reports a revision/dirty suffix. Treat this as misleading feedback for source builds, not evidence that the binaries differ. No repair is included in this review.</p>'
    evidence += _excerpt('rust/loopflow/src/lf/commands/machine.rs', 'report_version — unchanged from base', [(380, 388)])
    evidence += '<details><summary>Reproduction and provenance</summary><p><a href="remote-task-demo-output.txt">Full captured output</a> · <a href="remote-task-demo-capture.patch">Capture-only harness diff</a> · <a href="capture-remote-task-demo.py">Reproduction script</a> · <a href="remote-task-demo.md">Demo note</a> · <a href="work-on-another-machine-name.md">Current design</a>.</p><p>The harness copies the existing five tests to a temporary integration target, adds observations, and uses issue name <code>INF-123</code> for the public launches. Assertions stay in place. The temporary target is deleted afterward; product and published test sources are unchanged.</p>'
    evidence += _terminal('cargo build -p loopflow --bin lf\nuv run --no-sync python scratch/capture-remote-task-demo.py\n\n5 passed; 0 failed; finished in 13.39s\n\nBuilt CLI SHA-256:\n48241be36c6655b8d8123b64ea14bcae1e03d4fe3b2637487ceed7bddb56f8f1', 'Reproduction commands and captured result · no installation or live credentials')
    evidence += '<p>The missing Linear credential warning is expected in this isolated fixture; confirmed planning was retained. The simulated agent had only a synthetic fixture account. Temporary paths in the transcript have been cleaned up.</p></details></section>'
    page = '<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>LOO-412 — Remote Task review</title><style>' + css + '</style></head><body>' + opening + send + adopt + identity + preserve + evidence + '<footer>Local review artifact · Published product source unchanged · Source excerpts pinned to ' + HEAD + '</footer></main></body></html>'
    (ROOT / 'scratch/pr-review.html').write_text(page)


if __name__ == '__main__':
    main()

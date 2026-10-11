import urllib.request, pathlib
x5 = pathlib.Path("../scratch/prototypes/x5-goal.html").read_text()
style = x5[x5.rindex("<style>"):x5.index("</style></head>")]
h = urllib.request.urlopen("http://localhost:5001/cli").read().decode()
a = h.index("<main"); a = h.index(">", a) + 1; b = h.index("</main>")
_m = h[a:b]
CLI = _m[_m.index('<section id="flows"'):_m.index('<section id="start"')].replace(
    '<section id="flows" class="reading">',
    '<section id="flows" class="reading cli-part">\n<h2><code>lf</code>: the CLI for software engineering</h2>', 1)
def _drop(html, name):
    i = html.index("<b>" + name + "</b>"); a0 = html.rindex("<li>", 0, i); b0 = html.index("</li>", i) + 5
    return html[:a0] + html[b0:]
CLI = _drop(_drop(CLI, "Linear"), "Workflows")
for _o, _n in [("is free, open-source, and can be used", "can be used"), (" MIT licensed.", "")]:
    assert CLI.count(_o) == 1, _o; CLI = CLI.replace(_o, _n)
def items(rows): return "\n".join(f"<li><b>{n}</b> {t}</li>" for n, t in rows)

STOPS = ["start", "task-design", "design", "pursue", "demo", "ship", "end"]
def strip(now):
    i = STOPS.index(now)
    return '<p class="strip">' + "".join(f'<span class="{"done" if k < i else "now" if k == i else "next"}">{n}</span>' for k, n in enumerate(STOPS)) + "</p>"
def row(chip, cls=""):
    return f'<p class="trow"><b>Add CSV export</b><span class="pos {cls}">{chip}</span><span class="id">EXP-12</span></p>'
def fig(n, title, text, card):
    return f'<li><div class="capt"><span>Fig. {n}</span><h3>{title}</h3><p>{text}</p></div><div class="card">{card}</div></li>'
def turn(who, text, cls=""):
    return f'<p class="turn {cls}"><span>{who}</span>{text}</p>'
FIGS = "\n".join([
fig(1, "Be the designer.", "Your agent drafts the plan. What gets built is what you made of it.",
    row("design · waiting on you", "w")
    + turn("Agent", "Plan: an Export button that downloads every row in the table.")
    + turn("You", "only what’s on screen, with their filters on. and some of these tables are huge, it can’t just hang", "you")
    + turn("Agent", "Redrafted: Export downloads the filtered view. Rows stream, so a large table starts downloading at once.", "new")),
fig(2, "Experience using it yourself.", "It is built and waiting. You try it like a user would, and what you notice goes back in.",
    row("demo · waiting on you", "w")
    + turn("Agent", "Pull request #84 is open. Try it: open any table, press Export.")
    + turn("You", "tried it. dates come out in UTC, should be the account’s timezone", "you")
    + turn("Agent", "Fixed and rebuilt. Dates now use the account’s timezone. Ready to try again.", "new")),
fig(3, "Understand what gets built.", "Nothing is hidden from you and nothing is lost. The design you agreed, the change itself, and every conversation behind it stay with the Task.",
    row("landed", "ok")
    + '<ul class="kept"><li><b>Design</b>the plan as you left it</li><li><b>Pull request #84</b>the change, merged</li><li><b>9 Sessions</b>each step your agent took, whichever agent ran it</li></ul>'),
])
def wf(name, note, *parts):
    out = ""
    for part in parts:
        out += f'<b>{part[1:]}</b>' if part.startswith("@") else f'<i>{part}</i>' if part in ("start", "end") else f'<span>{part} →</span>'
    return f'<li><code>{name}</code><p class="line">{out}</p><small>{note}</small></li>'
WFS = "\n".join([
wf("feature", "The default. You jam on the design, and you try every change before it ships.", "start", "task-design", "@design", "pursue", "@demo", "ship", "end"),
wf("code", "One stop. Your agent builds straight from the task, and you try the change.", "start", "pursue", "@demo", "ship", "end"),
wf("research", "No pull request. Your agent investigates, and the task ends when you accept the findings.", "start", "research", "@findings", "end"),
])
YAML = """# .lf/workflows/feature.yaml
nodes:
  design: { skill: review-design }
  demo:   { skill: demo }
edges:
  - { from: start,  to: design, flow: task-design }
  - { from: design, to: design, flow: task-design }
  - { from: design, to: demo,   flow: pursue }
  - { from: demo,   to: demo,   flow: pursue }
  - { from: demo,   to: end,    flow: ship }"""
main = f'''
<section class="home-hero">
  <h1 aria-label="Loopflow is how you design, implement, demo and ship software with agents.">
  <span aria-hidden="true">Loopflow is how you <span class="rotating"><span>design</span><span>implement</span><span>demo</span><span>ship</span></span> software with agents.</span></h1>
  <div class="get"><a class="button solid" href="/download">Download for Mac</a><span class="or">or the CLI</span><div class="install-row"><pre tabindex="0" aria-label="Install the lf command line" class="term"><span class="p">$</span> curl -fsSL https://loopflow.studio/install.sh | sh</pre><button type="button" data-copy="curl -fsSL https://loopflow.studio/install.sh | sh" aria-live="polite" class="copy">Copy</button></div></div>
  <p class="cost">Free and open source (MIT)</p>
</section>
<section id="mac" class="x5 x6">
  <p class="slot"><span>Video goes here</span><span>A mock of the app stands in until it is recorded</span></p>
  <div class="win">
    <aside class="side">
      <h4>Your App</h4>
      <p class="on">Exports</p>
      <ul><li class="wait">Add CSV export</li><li>Filter exports by date</li><li class="wait">Schedule a weekly export</li><li>Export to Google Sheets</li></ul>
      <p>Onboarding</p>
      <ul><li>Invite a teammate</li><li>First-run checklist</li></ul>
      <p>Billing</p>
      <ul><li>Annual plans</li></ul>
    </aside>
    <div class="pane">
      <h2>Exports</h2>
      <p class="obj">People can get their data out of the app in the form they need it, without asking us.</p>
      <p class="lab">Workflow · <code>feature</code></p>
      <ol class="wf" aria-label="The feature workflow: start, design, demo, end">
        <li class="cap">start</li>
        <li class="edge"><b>task-design</b><i></i><pre>- kickoff</pre></li>
        <li class="node"><div><span>you</span><b>design</b></div><small>you review the plan</small></li>
        <li class="edge"><b>pursue</b><i></i><pre>- implement
- compress
- flow: refresh
- loop: implement
- pr-publish</pre></li>
        <li class="node"><div><span>you</span><b>demo</b></div><small>you try the change</small></li>
        <li class="edge"><b>ship</b><i></i><pre>- gate
- cmd: pr land -c</pre></li>
        <li class="cap">end</li>
      </ol>
      <p class="lab">tasks · 5</p>
      <ol class="tasks t6">
        <li><div><b>Add CSV export</b></div><span class="pos w">demo · waiting on you</span><span class="id">EXP-12</span></li>
        <li><div><b>Schedule a weekly export</b></div><span class="pos w">design · waiting on you</span><span class="id">EXP-15</span></li>
        <li><div><b>Filter exports by date</b></div><span class="pos">pursue · implement</span><span class="id">EXP-14</span></li>
        <li><div><b>Export to Google Sheets</b></div><span class="pos">start</span><span class="id">EXP-16</span></li>
        <li class="done"><div><b>Fix the timezone in exported dates</b></div><span class="pos ok">landed</span><span class="id">EXP-9</span></li>
      </ol>
    </div>
  </div>
</section>
<section class="pair system">
  <h2>Loopflow: a system for arranging coding sessions around long-lived waves and short-lived tasks.</h2>
  <div class="halves">
    <article>
      <p>Coding agents can do amazing things for you, but they ask a lot of focus from you. How can you, a human with a single locus of attention, stay in sync with an entire parallel agent swarm? Shipping tens of thousands of lines is the easy part. Making them lines worth keeping — lines you don’t revert next month — is the hard part.</p>
      <p>You don’t have to keep every session window open for fear of it disappearing, or remember why you wanted this massive change in the first place. Each task follows a workflow you choose, and each session you come to is one where your input really matters. Every task ends shipped.</p>
      <p class="more"><a href="/docs/waves">Tasks and Waves ↗</a><a href="/docs/authoring#workflows">Write a workflow ↗</a></p>
    </article>
    <article>
      <div class="card"><ul class="wfs">
{WFS}
      </ul></div>
    </article>
  </div>
</section>
<section class="pair clihero">
  <h2><code>lf</code>: the CLI for agent workflows.</h2>
  <div class="halves">
    <article>
      <p>To keep every session tied to its goal, Loopflow has to start each agent the same way: on the right machine, on the right login, with the right model, in the right worktree. <code>lf</code> is the command that does it.</p>
      <p>You don’t have to buy into tasks to use it. On its own, <code>lf</code> is a launcher for Claude Code, Codex, and OpenCode: use it anywhere you would start one of them.</p>
      <p>Private, secure remote jobs and telemetry made easy through <code>ssh</code>. Multi-subscription balancing: <code>lf</code> manages your logins, and moves to the next one when one runs out.</p>
      <p class="more"><a href="/cli">The CLI ↗</a><a href="/docs/authoring#flows">Skills and Flows ↗</a></p>
    </article>
    <article>
<pre tabindex="0" aria-label="lf commands" class="term"><span class="p">$</span> <b>lf</b> wt create csv-export     <span class="c"># a worktree and a branch</span>

<span class="p">$</span> <b>lf</b> --machine mini \\         <span class="c"># where it runs</span>
     --account claude=you@ \\  <span class="c"># whose login pays</span>
     -a claude:opus \\         <span class="c"># which agent and model</span>
     --wt csv-export \\        <span class="c"># in which worktree</span>
     --docs docs/api.md \\     <span class="c"># what it reads first</span>
     -b \\                     <span class="c"># headless, or -i to talk</span>
     graph pursue             <span class="c"># Skills in sequence: a Graph</span>

<span class="p">$</span> <b>lf</b> pr open                  <span class="c"># a draft PR, opened for you</span>
<span class="p">$</span> <b>lf</b> land --watch             <span class="c"># merge when checks pass; fix if not</span></pre>
    </article>
  </div>
</section>
<section id="works" class="works">
  <p class="eyebrow">Works with</p>
  <ul>
    <li><b><a href="https://claude.com/claude-code" target="_blank" rel="noopener">Claude Code</a> · <a href="https://openai.com/codex" target="_blank" rel="noopener">Codex</a> · <a href="https://opencode.ai" target="_blank" rel="noopener">OpenCode</a></b><span>the coding agent, on your own account</span></li>
    <li><b><a href="https://github.com" target="_blank" rel="noopener">GitHub</a></b><span>your code and your pull requests</span></li>
    <li><b><a href="https://linear.app" target="_blank" rel="noopener">Linear</a></b><span>your tasks are Linear issues</span></li>
    <li><b><a href="https://wisprflow.ai" target="_blank" rel="noopener">Wispr Flow</a></b><span>talk instead of typing</span></li>
    <li><b><a href="https://cmux.com" target="_blank" rel="noopener">cmux</a> · <a href="https://herdr.dev" target="_blank" rel="noopener">herdr</a></b><span><code>lf</code> runs inside them</span></li>
  </ul>
</section>
'''
extra = '''
.home-page .home-hero .actions.top{margin:26px 0 6px;gap:18px;align-items:center}.home-page .home-hero .actions.top span{font-size:13px;line-height:1.5;color:var(--text-secondary);max-width:520px}
.home-page .thesis{padding-top:56px}.home-page .thesis b{font-weight:600;color:var(--text)}
.home-page .pair h2{max-width:900px}.home-page .pair .halves .more{margin:0;border:0;padding:0}
.home-page .works .under{margin:22px 0 0;font:500 14px var(--font-mono)}.home-page .works .under a{color:var(--text);text-decoration:underline;text-decoration-color:#b9b1a6;text-underline-offset:4px}.home-page .works .under a:hover{color:var(--wine)}
.home-page .reading.closing{padding-top:8px}
.home-page .system h2{max-width:980px}
.home-page .works{padding-bottom:88px}
.home-page .clihero h2 code{font-size:.9em}.home-page .clihero article{min-width:0}.home-page .clihero .term{margin:0;font-size:12px;overflow-x:auto;max-width:100%}
.home-page #yours .say.req{margin-bottom:4px}.home-page #yours .reqs{margin:0 0 14px;padding-left:1.2em;font-size:18px;line-height:1.6;color:#333}.home-page #yours .reqs li{margin:0}.home-page #yours .reqs code{font-size:.88em}
.home-page #yours .say{max-width:760px;margin:0 0 14px}.home-page #yours .say code{font-size:.88em}.home-page #yours{padding-bottom:28px}
.home-page .pair .halves{margin-top:22px!important;align-items:start}.home-page .pair h2 em{font-style:italic}
.home-page .get{display:flex;flex-wrap:wrap;align-items:center;gap:12px 16px;margin-top:26px}.home-page .get .or{font:500 11px var(--font-mono);text-transform:uppercase;letter-spacing:.1em;color:var(--text-secondary)}.home-page .get .install-row{margin:0;flex:0 1 auto}.home-page .get .term{margin:0}.home-page .home-hero .cost{margin:12px 0 6px}.home-page .closing .get{margin-top:0}
.home-page .pair .halves article{padding:0!important;margin:0!important}
.home-page .thesis{padding:34px 0 8px;max-width:800px}.home-page .thesis p{font:400 clamp(18px,1.5vw,21px)/1.6 var(--font-sans);color:#2a2523;margin:0 0 20px}.home-page .thesis p:last-child{margin-bottom:0}
.home-page .pair .say:has(+ .say){margin-bottom:16px}
.home-page .cli-part{padding-top:72px}.home-page .cli-part h2{font-size:clamp(30px,3.2vw,42px);margin:0 0 26px}.home-page .cli-part h2 code{font-size:.9em}
.walk .turn{margin-top:12px!important;padding:10px 14px;border-radius:8px;background:#fff;border:1px solid var(--border);font-size:14px;line-height:1.55;color:#333}
.walk .turn span{display:block;font:500 10.5px var(--font-mono);text-transform:uppercase;letter-spacing:.1em;color:var(--text-secondary);margin-bottom:3px}
.walk .turn.you{background:#fbf2f4;border-color:#ecd3da;font:16px/1.5 var(--font-serif);color:#3a3532;margin-left:28px!important}.walk .turn.you span{color:var(--wine)}
.walk .turn.new{box-shadow:inset 3px 0 0 #1f6b36}
.walk .kept{list-style:none;margin:12px 0 0;padding:0;border:1px solid var(--border);border-radius:8px;background:#fff}
.walk .kept li{list-style:none;margin:0;padding:10px 14px;border-bottom:1px solid var(--border);font-size:13.5px;color:#444;display:grid;grid-template-columns:150px 1fr;gap:10px}.walk .kept li:last-child{border-bottom:0}.walk .kept b{color:var(--text);font-weight:600}
.ledger{display:grid;grid-template-columns:1fr 1fr;gap:0 56px;margin-top:34px}
.home-page .ledger .eyebrow{color:var(--burgundy);margin:0 0 10px}
.ledger ul{list-style:none;margin:0;padding:0;border-top:1px solid var(--border)}
.ledger li{list-style:none;margin:0;padding:10px 0;border-bottom:1px solid var(--border);font-size:16px}
.ledger div+div li{color:var(--text-secondary)}
.walk .figs{margin-top:26px!important}
@media (max-width:620px){.ledger{grid-template-columns:1fr;gap:26px}.walk .kept li{grid-template-columns:1fr;gap:2px}.walk .turn.you{margin-left:12px!important}}

.wh{width:100%;border-collapse:collapse;margin:30px 0 0;font-size:14.5px;line-height:1.55;background:none}
.home-page .wh th,.home-page .wh td{background:none;border:0;border-bottom:1px solid var(--border);padding:14px 22px 14px 0;text-align:left;vertical-align:top;color:#333}
.home-page .wh thead th{font:400 26px/1.1 var(--font-serif);color:var(--burgundy);border-bottom:1px solid #b9b1a6;padding-bottom:12px}
.wh thead th:last-child{font:500 11px/1.6 var(--font-mono)!important;text-transform:uppercase;letter-spacing:.1em;color:var(--text-secondary)!important;vertical-align:bottom}
.wh thead small{display:block;font:500 11px/1.6 var(--font-mono);text-transform:uppercase;letter-spacing:.1em;color:var(--text-secondary);margin-top:4px}
.home-page .wh tbody th{font:500 11px/1.9 var(--font-mono);text-transform:uppercase;letter-spacing:.1em;color:var(--text-secondary);white-space:nowrap;width:150px}
.wh td b{display:block;font-weight:600;color:var(--text)}.home-page .wh td.you:last-child{color:var(--wine);font-weight:600}.home-page .wh tbody th{font:400 24px/1.2 var(--font-serif)!important;text-transform:none!important;letter-spacing:0!important;color:var(--burgundy)!important;width:130px}.home-page .wh td:last-child{width:14%}
.home-page .wh td:last-child{font-size:13.5px;color:var(--text-secondary);width:24%}
.wh code{font:12.5px var(--font-mono);background:none;padding:0;color:var(--text)}
.pair .halves{border-top:0!important;padding-top:0!important;margin-top:44px!important;align-items:center}
@media (max-width:820px){.wh thead{display:none}.wh tr{display:block;padding:12px 0;border-bottom:1px solid var(--border)}.home-page .wh th,.home-page .wh td{display:block;border:0;padding:3px 0;width:auto!important}.wh td:nth-child(2)::before{content:"What · "}.wh td:nth-child(3)::before{content:"How · "}.wh td::before{font:500 10.5px var(--font-mono);text-transform:uppercase;letter-spacing:.1em;color:var(--burgundy)}}

.pair{padding-top:64px}.pair h2{font-size:clamp(30px,3.2vw,42px);margin:0 0 14px}
.home-page .pair .say{font-size:18px;line-height:1.6;color:#333;max-width:760px;margin:0}.pair .say b{color:var(--text);font-weight:600}
.pair .halves{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:36px 56px;margin:34px 0 22px;padding-top:30px;border-top:1px solid var(--border)}
.home-page .pair .eyebrow{color:var(--burgundy);margin:0 0 10px}
.pair h3{font:400 clamp(26px,2.6vw,34px)/1.12 var(--font-serif);letter-spacing:-.015em;margin:0 0 12px}
.pair article>p:not(.eyebrow){font-size:16px;line-height:1.6;color:#333;margin:0 0 20px}
.pair .card{background:#fdfbf7;border:1px solid #d9d4cc;border-radius:10px;padding:18px 20px;box-shadow:0 18px 40px -26px #722f3766}
.pair .card p{margin:0}.pair .lab{font:500 11px/1.6 var(--font-mono);text-transform:uppercase;letter-spacing:.1em;color:var(--text-secondary)}.pair .lab.down{margin-top:14px}
.pair .ramble{font:16.5px/1.6 var(--font-serif);color:#3a3532;margin-top:6px}
.pair .tl{list-style:none;margin:6px 0 0;padding:0;border:1px solid var(--border);border-radius:7px;background:#fff}.pair .tl li{list-style:none;margin:0;padding:9px 14px;border-bottom:1px solid var(--border);font-size:14.5px}.pair .tl li:last-child{border-bottom:0}.pair .tl .pick{font-weight:600;background:#fbf2f4;box-shadow:inset 3px 0 0 var(--wine)}.pair .tl small{display:block;font-weight:400;font-size:13px;line-height:1.5;color:#444;margin-top:3px}
.pair .wfs{list-style:none;margin:0;padding:0}.pair .wfs li{list-style:none;margin:0;padding:12px 0;border-bottom:1px solid var(--border)}.pair .wfs li:first-child{padding-top:0}.pair .wfs li:last-child{border-bottom:0;padding-bottom:0}
.pair .wfs code{font:500 13px var(--font-mono);color:var(--text);background:none;padding:0}
.pair .wfs .line{display:flex;flex-wrap:wrap;align-items:center;gap:6px;margin:8px 0 6px;font:11.5px var(--font-mono);color:var(--text-secondary)}
.pair .wfs .line i{font-style:normal;text-transform:uppercase;letter-spacing:.08em;font-size:10.5px}
.pair .wfs .line b{font:500 11.5px var(--font-mono);padding:3px 9px;border-radius:6px;background:linear-gradient(180deg,#fbf2f4,#f1dce2);box-shadow:inset 0 0 0 1px #722f371f;color:var(--burgundy)}
.pair .wfs .line b::after{content:" →";color:var(--text-secondary);font-weight:400}
.pair .wfs small{font-size:13.5px;color:#444}
@media (max-width:820px){.pair .halves{grid-template-columns:1fr}}

.walk .lab.down{margin-top:14px!important}.walk .steps{white-space:pre-wrap}
.conf{padding-top:64px}.conf h2{font-size:clamp(30px,3.2vw,42px);margin:0 0 14px}
.home-page .conf .say{font-size:18px;line-height:1.6;color:#333;max-width:720px;margin:0 0 12px}.conf .say b{color:var(--text);font-weight:600}
.conf .shared{list-style:none;margin:22px 0 0;padding:0;display:grid;grid-template-columns:repeat(auto-fit,minmax(210px,1fr));border-top:1px solid var(--border);border-bottom:1px solid var(--border)}.conf .shared li{list-style:none;margin:0;padding:16px 18px 16px 0;display:grid;gap:5px}.conf .shared code{font:500 13px var(--font-mono);color:var(--text);background:none;padding:0}.conf .shared span{font-size:13.5px;line-height:1.5;color:#444}.conf .two{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:28px 40px;align-items:start;margin:28px 0 22px}
.conf .term{margin:0;font-size:12.5px}
.conf .wfs{list-style:none;margin:0;padding:0;border-top:1px solid var(--border)}
.conf .wfs li{list-style:none;margin:0;padding:14px 0;border-bottom:1px solid var(--border)}
.conf .wfs code{font:500 13px var(--font-mono);color:var(--text);background:none;padding:0}
.conf .wfs .line{display:flex;flex-wrap:wrap;align-items:center;gap:6px;margin:8px 0 6px;font:11.5px var(--font-mono);color:var(--text-secondary)}
.conf .wfs .line i{font-style:normal;text-transform:uppercase;letter-spacing:.08em;font-size:10.5px}
.conf .wfs .line b{font:500 11.5px var(--font-mono);padding:3px 9px;border-radius:6px;background:linear-gradient(180deg,#fbf2f4,#f1dce2);box-shadow:inset 0 0 0 1px #722f371f;color:var(--burgundy)}
.conf .wfs .line b::after{content:" →";color:var(--text-secondary);font-weight:400}
.conf .wfs small{font-size:13.5px;color:#444}
@media (max-width:820px){.conf .two{grid-template-columns:1fr}}

.walk{padding-top:64px}.works{padding-top:56px}.home-page .works .eyebrow{color:var(--burgundy);margin-bottom:14px}.works ul{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:0;border-top:1px solid var(--border);border-bottom:1px solid var(--border)}.works li{list-style:none;margin:0;padding:18px 18px 18px 0;display:grid;gap:4px}.works b{font:500 15px var(--font-sans)}.works b a{color:var(--text);text-decoration:underline;text-decoration-color:#b9b1a6;text-underline-offset:4px}.works b a:hover{color:var(--wine);text-decoration-color:var(--wine)}.works span{font-size:13px;line-height:1.5;color:var(--text-secondary)}.walk h2{font-size:clamp(30px,3.2vw,42px);margin:0 0 10px}.home-page .walk .say{font-size:18px;line-height:1.6;color:#333;margin:0 0 30px;max-width:720px}
.home-page .walk .say:not(.lead){margin-bottom:12px}.walk .figs{list-style:none;margin:0;padding:0;border-top:1px solid var(--border)}
.walk .figs>li{list-style:none;margin:0;display:grid;grid-template-columns:minmax(0,5fr) minmax(0,7fr);gap:24px 56px;align-items:center;padding:34px 0;border-bottom:1px solid var(--border)}
.walk .capt span{font:500 11px var(--font-mono);text-transform:uppercase;letter-spacing:.1em;color:var(--burgundy)}
.walk .capt h3{font:400 clamp(26px,2.6vw,34px)/1.12 var(--font-serif);letter-spacing:-.015em;margin:8px 0 10px}
.walk .capt p{font-size:16px;line-height:1.6;color:#333;margin:0;max-width:44ch}
.walk .card{background:#fdfbf7;border:1px solid #d9d4cc;border-radius:10px;padding:18px 20px;box-shadow:0 18px 40px -26px #722f3766;min-width:0}
.walk .card p{margin:0}.walk .trow{display:flex;align-items:center;gap:10px;font-size:15px;flex-wrap:wrap}.walk .trow b{font-weight:600;margin-right:auto}
.walk .pos{font:500 11px var(--font-mono);padding:3px 9px;border-radius:20px;background:#ece8e0;color:#5a524a;white-space:nowrap}
.walk .pos.w{background:#f6dfe7;color:var(--wine)}.walk .pos.ok{background:#dcefe0;color:#1f6b36}.walk .id{font:12px var(--font-mono);color:var(--text-secondary)}
.walk .strip{display:flex;flex-wrap:wrap;gap:5px;margin-top:14px!important}
.walk .strip span{font:500 10.5px var(--font-mono);padding:3px 8px;border-radius:5px;background:#f1ede6;color:#a39b90}
.walk .strip .done{background:#e6f1e8;color:#1f6b36}.walk .strip .now{background:var(--wine);color:#fff}
.walk .doc{margin-top:14px;padding:12px 14px;background:#fff;border:1px solid var(--border);border-radius:7px;font-size:13.5px;line-height:1.55;color:#333}
.walk .doc b{display:block;margin-bottom:4px}.walk .doc p+p{margin-top:6px}
.walk .steps{margin:14px 0 0;padding:12px 14px;background:#fff;border:1px solid var(--border);border-radius:7px;font:12px/1.75 var(--font-mono);color:#a39b90;white-space:pre;overflow:auto}
.walk .steps .ok{color:#1f6b36}.walk .steps .run{color:var(--wine)}
.walk .sess{margin-top:12px!important;font:12px/1.6 var(--font-mono);color:var(--text-secondary)}.walk .sess b{color:var(--text);font-weight:500;margin-right:8px}
.walk .ramble{font:16.5px/1.6 var(--font-serif);color:#3a3532;margin-top:6px!important}.walk .tl{list-style:none;margin:6px 0 0;padding:0;border:1px solid var(--border);border-radius:7px;background:#fff}.walk .tl li{list-style:none;margin:0;padding:9px 14px;border-bottom:1px solid var(--border);font-size:14.5px}.walk .tl li:last-child{border-bottom:0}.walk .tl .pick{font-weight:600;background:#fbf2f4;box-shadow:inset 3px 0 0 var(--wine)}.walk .tl small{display:block;font-weight:400;font-size:13px;line-height:1.5;color:#444;margin-top:3px}.walk .wname{font:400 28px/1.1 var(--font-serif)}.walk .wobj{font:16px/1.5 var(--font-serif);color:#3a3532;margin:6px 0 14px!important}
.walk .lab{font:500 11px/1.6 var(--font-mono);text-transform:uppercase;letter-spacing:.1em;color:var(--text-secondary)}
.walk .krs{list-style:none;padding:0;margin:4px 0 0}.walk .krs li{list-style:none;margin:0;padding:3px 0 3px 24px;position:relative;font-size:14px}
.walk .krs li::before{content:"";position:absolute;left:2px;top:9px;width:11px;height:11px;border:1.5px solid #b9b1a6;border-radius:50%}
.walk .plain{max-width:900px;margin-top:40px}.walk .more{max-width:900px}
@media (max-width:820px){.walk .figs>li{grid-template-columns:1fr;padding:26px 0}}

.x6{padding-top:36px}.home-page .reading .say{font-size:18px;line-height:1.6;color:#333;max-width:700px;margin:0}.home-page .closing .actions{margin-bottom:14px}
.x6 .slot{display:flex;justify-content:space-between;gap:6px 24px;flex-wrap:wrap;margin:0;padding:10px 16px;font:500 11px/1.6 var(--font-mono);text-transform:uppercase;letter-spacing:.1em;color:var(--burgundy);background:#f4f0ea;border:1px solid #d9d4cc;border-bottom:0;border-radius:12px 12px 0 0}
.x6 .slot span+span{color:var(--text-secondary)}.x6 .win{border-radius:0 0 12px 12px}
.x6 .pane h2{font:400 34px/1.1 var(--font-serif);color:var(--text);margin:0 0 8px}
.x6 .side .wait::before{content:"";display:inline-block;width:6px;height:6px;border-radius:50%;background:#c0264f;margin:0 7px 1px -13px}
.x6 .wf{align-items:flex-start;padding:22px 18px}
.x6 .wf .cap{padding-top:24px;display:grid;gap:10px;justify-items:center}
.x6 .wf .edge{gap:0;justify-items:center}.x6 .wf .edge b{line-height:24px}.x6 .wf .edge i{width:100%;margin:8px 0 14px}
.x6 .wf .edge pre{margin:0;padding:0;background:none;text-align:left;font:11.5px/1.65 var(--font-mono);color:#444;white-space:pre;overflow:visible}
.x6 .wf .node{background:none;box-shadow:none;padding:0;gap:6px;justify-items:center}
.x6 .wf .node div{width:100%;height:68px;display:grid;align-content:center;gap:1px;background:linear-gradient(180deg,#fbf2f4,#f1dce2);box-shadow:inset 0 0 0 1px #722f371f;border-radius:10px}
.x6 .wf u{margin-top:8px}
.x6 .wf u{display:inline-block;white-space:nowrap;text-decoration:none;font:500 10.5px var(--font-mono);letter-spacing:.04em;padding:2px 8px;border-radius:20px;background:#ece8e0;color:#5a524a;text-transform:none}
.x6 .wf u.w{background:#c0264f;color:#fff}
.x6 .t6 li{grid-template-columns:26px minmax(0,1fr) auto auto;align-items:center}
.x6 .t6 .pos{font:500 11px var(--font-mono);padding:3px 9px;border-radius:20px;background:#ece8e0;color:#5a524a;white-space:nowrap}
.x6 .t6 .pos.w{background:#f6dfe7;color:var(--wine)}.x6 .t6 .pos.ok{background:#dcefe0;color:#1f6b36}
.x6 .t6 .id{font:12px var(--font-mono);color:var(--text-secondary)}.x6 .t6 .done b{font-weight:400;color:var(--text-secondary)}
@media (max-width:900px){.x6 .wf{align-items:stretch}.x6 .wf .cap{padding-top:0}.x6 .wf .edge i{width:2px;margin:8px auto}.x6 .t6 li{grid-template-columns:22px minmax(0,1fr)}.x6 .t6 .pos{grid-column:2;justify-self:start}.x6 .t6 .id{display:none}}
'''
out = (h[:a] + main + h[b:]).replace('"/static/', '"../../website/static/').replace('href="/', 'href="http://localhost:5001/').replace("</head>", style + extra + "</style></head>")
pathlib.Path("../scratch/prototypes/x11-redesign.html").write_text(out)

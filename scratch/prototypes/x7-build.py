import urllib.request, pathlib
x5 = pathlib.Path("../scratch/prototypes/x5-goal.html").read_text()
style = x5[x5.rindex("<style>"):x5.index("</style></head>")]
h = urllib.request.urlopen("http://localhost:5001/cli").read().decode()
a = h.index("<main"); a = h.index(">", a) + 1; b = h.index("</main>")
def items(rows): return "\n".join(f"<li><b>{n}</b> {t}</li>" for n, t in rows)

STOPS = ["start", "task-design", "design", "pursue", "demo", "ship", "end"]
def strip(now):
    i = STOPS.index(now)
    return '<p class="strip">' + "".join(f'<span class="{"done" if k < i else "now" if k == i else "next"}">{n}</span>' for k, n in enumerate(STOPS)) + "</p>"
def row(chip, cls=""):
    return f'<p class="trow"><b>Add CSV export</b><span class="pos {cls}">{chip}</span><span class="id">EXP-12</span></p>'
def fig(n, title, text, card):
    return f'<li><div class="capt"><span>Fig. {n}</span><h3>{title}</h3><p>{text}</p></div><div class="card">{card}</div></li>'
FIGS = "\n".join([
fig(1, "A rambling idea", "Say it however it comes out, typed or dictated. Half-formed is fine.",
    '<p class="lab">You</p><p class="ramble">ok so people keep emailing us asking for their data. I want them to just get it out themselves. csv probably, with whatever filters they have on. maybe a weekly one that shows up by itself? sheets would be cool at some point. oh and the dates in the old export are wrong, that needs fixing first</p>'),
fig(2, "The work, written down", "Loopflow writes it down in words you already use: a Wave, which is a persistent workstream with an objective, and the Tasks under it. Each Task states what will be true when it is done. Change any of it, and that is what your agent works from.",
    '<p class="wname">Exports</p><p class="wobj">People can get their data out of the app in the form they need it, without asking us.</p><ul class="tl"><li>Fix the timezone in exported dates</li><li class="pick">Add CSV export<small>Done when anyone can download the table they are looking at as a CSV file, with their filters applied.</small></li><li>Schedule a weekly export</li><li>Export to Google Sheets</li></ul>'),
fig(3, "You jam on the design.", "Your agent drafts a plan and brings it to you. You go back and forth: push on it, answer its questions, change your mind. It redrafts until the design is yours.",
    row("design · waiting on you", "w") + strip("design") + '<div class="doc"><b>Plan</b><p>An Export button in the table header. Rows stream as CSV, so large tables do not time out.</p><p><i>Open question:</i> should archived rows be included?</p></div>'),
fig(4, "It gets built without you.", "The next Flow implements the plan, tidies it, catches up with the rest of the code, and opens the pull request. Each step is a Session of its own.",
    row("pursue · flow: refresh") + strip("pursue") + '<pre class="steps"><span class="ok">✓ implement</span>\n<span class="ok">✓ compress</span>\n<span class="run">● flow: refresh</span>\n○ loop: implement\n○ pr-publish</pre>'),
fig(5, "You try it.", "The Task waits at demo, marked, until you have tried the change. Not right? Say so, and it goes back through.",
    row("demo · waiting on you", "w") + strip("demo") + '<div class="doc"><b>Pull request #84 is open</b><p>Try it: open any table, press Export, and check the dates in the file.</p></div>'),
fig(6, "It ships.", "Checks run, then it merges. Every Session that got it there stays under the Task, whichever agent ran it.",
    row("landed", "ok") + strip("end") + '<pre class="steps"><span class="ok">✓ gate</span>\n<span class="ok">✓ cmd: pr land -c</span></pre><p class="sess"><b>9 Sessions</b> kickoff · design · implement · compress · realign · implement · pr-publish · demo · gate</p>'),
])
main = f'''
<section class="home-hero">
  <h1 aria-label="Loopflow is where you design, implement, demo and ship software with agents.">
  <span aria-hidden="true">Loopflow is where you <span class="rotating"><span>design</span><span>implement</span><span>demo</span><span>ship</span></span> software with agents.</span></h1>
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
      <p class="lab">Tasks · 5</p>
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
<section class="walk">
  <h2>Loopflow is a software instrument.</h2>
  <p class="say">Coding with agents is about empowering people, not delegating the thinking to robots. An instrument is something you play, and what comes out is yours. Your agent does the building. You decide what to make, and whether it is right.</p>
  <p class="say lead">Here is how it goes, from a rambling idea to a set of merged pull requests.</p>
  <ol class="figs">
{FIGS}
  </ol>
  <ul class="plain">
    <li><b>Every Task ends shipped</b> Loopflow uses configurable workflows to ensure that Tasks are designed thoughtfully, built simply, shipped, and validated thoroughly, all automatically.</li>
    <li><b>You choose the stops</b> The workflow is a file. Steer every step, come in at design and demo, or let it run, project by project.</li>
    <li><b>Underneath</b> A native Mac terminal, running Claude Code, Codex, or OpenCode on your own account, with <code>lf</code> included.</li>
  </ul>
  <p class="more"><a href="/cli">Underneath, it is all <code>lf</code> ↗</a><a href="/docs/waves">Tasks and Waves ↗</a></p>
</section>
<section id="works" class="works">
  <p class="eyebrow">Works with</p>
  <ul>
    <li><b>Claude Code · Codex · OpenCode</b><span>the coding agent, on your own account</span></li>
    <li><b>GitHub</b><span>your code and your pull requests</span></li>
    <li><b>Linear</b><span>your Tasks are Linear issues</span></li>
    <li><b>Wispr Flow</b><span>talk instead of typing</span></li>
    <li><b>cmux · herdr</b><span><code>lf</code> runs inside them</span></li>
  </ul>
</section>
<section id="yours" class="reading">
  <h2>Free and open source (MIT).</h2>
  <ul class="plain">
{items([
("What you need", "A coding agent (Claude Code, Codex, or OpenCode) and a GitHub account."),
("What you get", "We’ll get you building whatever software you want like a software engineer, by guiding you through workflows that get the most out of you and ensure high quality software, not slop."),
("Your code", "It stays in your GitHub and on your Mac. Nothing is hosted, so there is nothing to move off later."),
("Change whatever you want", "Skills are Markdown and Flows are YAML. Edit them or use the built-ins."),
])}
  </ul>
</section>
<section id="start" class="reading closing">
  <p class="eyebrow">Install</p>
  <div class="actions"><a class="button solid" href="#">Download for Mac</a></div>
  <p class="cost">Free and open source. The coding agent it drives (Claude Code, Codex, or OpenCode) runs on your own account and has its own cost.</p>
</section>
'''
extra = '''
.walk{padding-top:64px}.works{padding-top:56px}.home-page .works .eyebrow{color:var(--burgundy);margin-bottom:14px}.works ul{list-style:none;margin:0;padding:0;display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:0;border-top:1px solid var(--border);border-bottom:1px solid var(--border)}.works li{list-style:none;margin:0;padding:18px 18px 18px 0;display:grid;gap:4px}.works b{font:500 15px var(--font-sans)}.works span{font-size:13px;line-height:1.5;color:var(--text-secondary)}.walk h2{font-size:clamp(30px,3.2vw,42px);margin:0 0 10px}.home-page .walk .say{font-size:18px;line-height:1.6;color:#333;margin:0 0 30px;max-width:720px}
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
out = (h[:a] + main + h[b:]).replace('"/static/', '"../../website/static/').replace("</head>", style + extra + "</style></head>")
pathlib.Path("../scratch/prototypes/x7-follow.html").write_text(out)

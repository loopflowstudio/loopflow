import urllib.request, pathlib
x5 = pathlib.Path("../scratch/prototypes/x5-goal.html").read_text()
style = x5[x5.rindex("<style>"):x5.index("</style></head>")]
h = urllib.request.urlopen("http://localhost:5001/cli").read().decode()
a = h.index("<main"); a = h.index(">", a) + 1; b = h.index("</main>")
def items(rows): return "\n".join(f"<li><b>{n}</b> {t}</li>" for n, t in rows)
main = f'''
<section class="home-hero">
  <h1 aria-label="Loopflow is where you design, implement, demo and ship software with agents.">
  <span aria-hidden="true">Loopflow is where you <span class="rotating"><span>design</span><span>implement</span><span>demo</span><span>ship</span></span> software with agents.</span></h1>
</section>
<section id="mac" class="x5 x6">
  <p class="slot"><span>Video goes here</span><span>A mock of the app stands in until it is recorded</span></p>
  <div class="win">
    <aside class="side">
      <h4>your-app</h4>
      <p class="on">exports</p>
      <ul><li class="wait">Add CSV export</li><li>Filter exports by date</li><li class="wait">Schedule a weekly export</li><li>Export to Google Sheets</li></ul>
      <p>onboarding</p>
      <ul><li>Invite a teammate</li><li>First-run checklist</li></ul>
      <p>billing</p>
      <ul><li>Annual plans</li></ul>
    </aside>
    <div class="pane">
      <h2>exports</h2>
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
<section class="reading">
  <h2>Every Task ends shipped.</h2>
  <p class="say">Loopflow uses configurable workflows to ensure that Tasks are designed thoughtfully, built simply, shipped, and validated thoroughly, all automatically.</p>
  <ul class="plain">
{items([
("It starts with a goal", "A Wave is a goal that outlasts any one conversation, like getting people’s data out of your app. It keeps its key results, its plan, and its memory."),
("One Task, one outcome", "“Add CSV export” is a Task. It ends in one pull request, and it is done when that merges."),
("Your agent plans, you review the design", "The Task’s workflow starts its first Flow. Your agent drafts a plan and stops. This stop is yours."),
("It gets built without you", "The next Flow implements the plan, tidies it, catches up with the rest of the code, and opens the pull request. Each step is a Session of its own."),
("You try it", "The Task waits at demo, marked, until you have tried the change. Send it back as often as it takes."),
("It ships", "Checks run, then it merges. Every Session that got it there is kept under the Task, whichever agent ran it."),
("You choose the stops", "The workflow is a file. Steer every step, come in at design and demo, or let it run, project by project."),
("Underneath", "A native Mac terminal, running Claude Code, Codex, or OpenCode on your own account, with <code>lf</code> included."),
])}
  </ul>
  <p class="more"><a href="/cli">Underneath, it is all <code>lf</code> ↗</a><a href="/docs/waves">Tasks and Waves ↗</a></p>
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
pathlib.Path("../scratch/prototypes/x6-video.html").write_text(out)

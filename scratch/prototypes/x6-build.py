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
        <li class="cap">start<u>1</u></li>
        <li class="edge"><b>task-design</b><i></i><small>kickoff</small></li>
        <li class="node"><span>you</span><b>design</b><small>you review the plan</small><u class="w">1 waiting</u></li>
        <li class="edge"><b>pursue</b><i></i><small>implement · compress · refresh · implement ↺ · pr-publish</small><u>1 running</u></li>
        <li class="node"><span>you</span><b>demo</b><small>you try the change</small><u class="w">1 waiting</u></li>
        <li class="edge"><b>ship</b><i></i><small>gate · pr land</small></li>
        <li class="cap">end<u>1</u></li>
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
  <ul class="plain">
{items([
("A terminal", "At its most basic, Loopflow for Mac is a terminal. Open a repository and start a Session with Claude Code, Codex, or OpenCode, the same way every time."),
("Skills", "Start a Session from a Skill instead of an empty prompt, so you stop retyping how you like work done. Pick one when you open the Session."),
("Sessions", "Every conversation is kept, whichever agent ran it. Close the window and it is still there when you come back."),
("Waves and Tasks", "The sidebar is your work, not a list of chats: Waves (long-lived streams), the Tasks under each (concrete outcomes, i.e. PRs), and the Sessions under each Task."),
("One Task, one copy of the code", "Each Task works in its own copy, on its own branch, toward one pull request, so many agents can work without colliding."),
("Workflows", "A Workflow is the stages a Task goes through to become a shipped and validated PR. Two stops are yours, design and demo. Between them a Flow runs, and your agent does each step in a Session of its own."),
("Waiting on you", "A Task that needs you is marked, so you look where you are needed and leave the rest running."),
("Side by side", "Split the window to follow several Sessions, or several Tasks, at once."),
])}
  </ul>
  <p class="more"><a href="/cli">Underneath, it is all <code>lf</code> ↗</a><a href="/docs/waves">Tasks and Waves ↗</a></p>
</section>
<section id="yours" class="reading">
  <h2>It is yours.</h2>
  <ul class="plain">
{items([
("What you need", "A coding agent (Claude Code, Codex, or OpenCode) and a GitHub account."),
("What you get", "We’ll get you building whatever software you want like a software engineer, by guiding you through workflows that get the most out of you and ensure high quality software, not slop."),
("Your code", "It stays in your GitHub and on your Mac. Nothing is hosted, so there is nothing to move off later."),
("Change whatever you want", "Skills are Markdown and Flows are YAML. Edit them or use the built-ins. MIT licensed."),
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
.x6{padding-top:36px}.home-page .closing .actions{margin-bottom:14px}
.x6 .slot{display:flex;justify-content:space-between;gap:6px 24px;flex-wrap:wrap;margin:0;padding:10px 16px;font:500 11px/1.6 var(--font-mono);text-transform:uppercase;letter-spacing:.1em;color:var(--burgundy);background:#f4f0ea;border:1px solid #d9d4cc;border-bottom:0;border-radius:12px 12px 0 0}
.x6 .slot span+span{color:var(--text-secondary)}.x6 .win{border-radius:0 0 12px 12px}
.x6 .pane h2{font:400 34px/1.1 var(--font-serif);color:var(--text);margin:0 0 8px}
.x6 .side .wait::before{content:"";display:inline-block;width:6px;height:6px;border-radius:50%;background:#c0264f;margin:0 7px 1px -13px}
.x6 .wf li{position:relative}.x6 .wf{padding-bottom:40px}
.x6 .wf u{position:absolute;left:50%;transform:translateX(-50%);bottom:-28px;white-space:nowrap;text-decoration:none;font:500 10.5px var(--font-mono);letter-spacing:.04em;padding:2px 8px;border-radius:20px;background:#ece8e0;color:#5a524a;text-transform:none}
.x6 .wf u.w{background:#c0264f;color:#fff}
.x6 .t6 li{grid-template-columns:26px minmax(0,1fr) auto auto;align-items:center}
.x6 .t6 .pos{font:500 11px var(--font-mono);padding:3px 9px;border-radius:20px;background:#ece8e0;color:#5a524a;white-space:nowrap}
.x6 .t6 .pos.w{background:#f6dfe7;color:var(--wine)}.x6 .t6 .pos.ok{background:#dcefe0;color:#1f6b36}
.x6 .t6 .id{font:12px var(--font-mono);color:var(--text-secondary)}.x6 .t6 .done b{font-weight:400;color:var(--text-secondary)}
@media (max-width:900px){.x6 .wf u{position:static;transform:none;justify-self:center;margin:6px auto 0;display:inline-block}.x6 .wf{padding-bottom:22px}.x6 .t6 li{grid-template-columns:22px minmax(0,1fr)}.x6 .t6 .pos{grid-column:2;justify-self:start}.x6 .t6 .id{display:none}}
'''
out = (h[:a] + main + h[b:]).replace('"/static/', '"../../website/static/').replace("</head>", style + extra + "</style></head>")
pathlib.Path("../scratch/prototypes/x6-video.html").write_text(out)

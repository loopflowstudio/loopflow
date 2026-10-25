"""Assemble the homepage takes from the rendered current homepage.

Run from the repository root: uv run --project website python scratch/website-takes/build.py
"""

import re
from pathlib import Path

HERE = Path(__file__).parent
current = (HERE / "current.html").read_text()

head_and_header = current.split('<main id="main-content" class="home">')[0]
footer = "</main>" + current.split("</main>", 1)[1]
teams = re.search(r'<section id="mac".*?</section>\s*</section>', current, re.S)
teams = re.search(r'<section id="mac" class="appmock duo">.*?</figure>\s*</section>', current, re.S).group(0)
works = re.search(r'<section id="works".*?</section>', current, re.S).group(0)
install = re.search(r'<div class="install-row">.*?</div>', current, re.S).group(0)


def page(name: str, title: str, body: str) -> str:
    html = head_and_header + '<main id="main-content" class="home">' + body + footer
    html = re.sub(r"<title>.*?</title>", f"<title>Loopflow — {title}</title>", html)
    html = html.replace('href="/static/', 'href="../../website/static/').replace('src="/static/', 'src="../../website/static/')
    html = html.replace("</head>", '<link rel="stylesheet" href="takes.css">\n</head>')
    html = html.replace("<body>", f'<body>\n<p class="take-bar"><a href="index.html">← all takes</a><span>{name}</span></p>')
    return html


SKILL_LINE = "npx skills add loopflowstudio/loopflow --skill loopflow -g -y"


def tell_row(line: str = SKILL_LINE) -> str:
    return (
        '<div class="install-row"><pre tabindex="0" class="term"><span class="p">$</span> '
        f'{line}</pre><button type="button" data-copy="{line}" class="copy">Copy</button></div>'
    )


TRANSCRIPT = """<div class="tx" role="img" aria-label="A Claude Code session handing work to lf runs and staying free to change course">
<p class="tx-top"><span>claude · opus</span><span>~/src/etude</span></p>
<p class="you">› add CPU pods for templates with no GPU, and backfill the pod column</p>
<p class="say">Two pieces. I’ll run each in the background on its own branch, and stay here with you.</p>
<p class="run"><b>lf</b> launch "rent CPU pods for templates with no GPU"</p>
<p class="out ok">ETU-14  launched · feature · design</p>
<p class="run"><b>lf</b> launch "backfill pod kind for old runs" -a codex</p>
<p class="out ok">ETU-15  launched · code · implement</p>
<p class="run"><b>lf</b> open --task ETU-14</p>
<p class="out dim">  opened in tmux window 2, beside this one</p>
<p class="say">Both running: ETU-14 is designing, ETU-15 is implementing. ETU-14’s own conversation is open in the next window if you want to talk to it.</p>
<p class="you">› wait, the backfill should keep the old column for a release</p>
<p class="say">Stopping ETU-15 before its next step and passing that on.</p>
<p class="run"><b>lf</b> task interrupt ETU-15</p>
<p class="run"><b>lf</b> comment ETU-15 "keep the old column for one release"</p>
<p class="run"><b>lf</b> launch ETU-15</p>
<p class="out dim">  reads your comment · branch etu-15 · 2 commits</p>
<p class="out bad">ETU-14  implement · stopped   claude you@ hit its limit</p>
<p class="say">ETU-14 ran out of login mid-implement. Its design and three commits are on the branch, so I’m restarting it on your other login.</p>
<p class="run"><b>lf</b> launch ETU-14 --account claude=work@</p>
<p class="out ok">ETU-15  ship · landed     #418</p>
<p class="out ok">ETU-14  ship · published  #419</p>
<p class="say">Both are up: <u>#418</u> landed with the old column kept, <u>#419</u> is ready for you to try.</p>
</div>"""


# ---------------------------------------------------------------- Take A

A = f"""<section class="home-hero ta">
  <p class="eyebrow">Take A · transcript-led</p>
  <h1>The CLI your agents use to get work done.</h1>
  <p class="sub">Tell your coding agent to use <code>lf</code>. It hands the work to background runs, each on its own branch, and stays in the conversation with you, free to answer, change course, or stop a run mid-step.</p>
  <div class="tell"><p class="lbl">Give your agent the skill</p>{tell_row()}<p class="lbl">or install the CLI</p>{install}</div>
</section>
<section class="ta-shot">{TRANSCRIPT}
  <p class="cap">One conversation in Claude Code. It never did the work itself, so it was free when you changed your mind, and nothing was lost when a run died.</p>
</section>
<section class="pair">
  <h2>Every phase has a contract.</h2>
  <div class="halves">
    <article>
      <p>A phase is a Skill: a Markdown file in your repository that says what to do and what to leave behind. A Graph puts phases in order. Workflows say where you come in.</p>
      <p>Nothing important lives only in an agent’s context. Each phase leaves its result on disk, so when an agent stops, the next one starts from there: work that outlives your agents.</p>
      <p class="more"><a href="/docs/authoring">Write a Skill ↗</a><a href="/docs/authoring#graphs">Graphs ↗</a></p>
    </article>
    <article>
<pre tabindex="0" class="term"><span class="c"># .lf/graphs/ship-api.yaml</span>
- design      <span class="c"># leaves scratch/&lt;task&gt;.md</span>
- implement   <span class="c"># leaves commits on the branch</span>
- gate        <span class="c"># leaves passing checks</span>
- ship        <span class="c"># leaves a pull request</span></pre>
    </article>
  </div>
</section>
<section class="pair">
  <h2>And when it’s more than your agents: your team’s.</h2>
  <p class="lede">The same Tasks your agent files are the plan your team reads, in Linear or a shared Git ref. One owner, one worktree, one PR each, so nobody’s agents take the same work.</p>
</section>
{teams}
{works}
<section class="belief"><p>Scale together.</p></section>"""


# ---------------------------------------------------------------- Take B

PHASES = """<div class="phases" role="img" aria-label="Four phases, each handing an artifact to the next; one agent fails and another continues">
  <div class="ph"><p class="nm">design</p><p class="ag">claude · you@</p><p class="art">scratch/cpu-pods.md</p></div>
  <i class="ar"></i>
  <div class="ph fail"><p class="nm">implement</p><p class="ag"><s>claude · you@</s> limit</p><p class="ag new">claude · work@</p><p class="art">etu-14 · 3 commits</p></div>
  <i class="ar"></i>
  <div class="ph"><p class="nm">gate</p><p class="ag">codex</p><p class="art">checks pass</p></div>
  <i class="ar"></i>
  <div class="ph you"><p class="nm">demo</p><p class="ag">you</p><p class="art">#419</p></div>
</div>"""

B = f"""<section class="home-hero tb">
  <p class="eyebrow">Take B · contract-led</p>
  <h1>Break the work into phases. Let any agent finish it.</h1>
  <p class="sub"><code>lf</code> is the CLI your agents use to get work done. Each phase leaves its result on disk, so when an agent stops, the next one starts from there.</p>
  <div class="tell">{install}<p class="lbl">Then tell your agent: <code>use lf to break this into Tasks</code></p></div>
</section>
<section class="tb-shot">{PHASES}
  <p class="cap">The <code>feature</code> workflow. A login ran out during implement; the next agent read the design and the branch and kept going.</p>
</section>
<section class="pair">
  <h2>Every phase has a contract.</h2>
  <div class="halves">
    <article>
      <p>A phase is a Skill: a Markdown file in your repository that says what to do and what to leave behind. A Graph puts phases in order. Workflows say where you come in.</p>
      <p>Nothing important lives only in an agent’s context. That is the whole trick: work that outlives your agents.</p>
      <p class="more"><a href="/docs/authoring">Write a Skill ↗</a><a href="/docs/authoring#graphs">Graphs ↗</a></p>
    </article>
    <article>
<pre tabindex="0" class="term"><span class="c"># .lf/graphs/ship-api.yaml</span>
- design      <span class="c"># leaves scratch/&lt;task&gt;.md</span>
- implement   <span class="c"># leaves commits on the branch</span>
- gate        <span class="c"># leaves passing checks</span>
- ship        <span class="c"># leaves a pull request</span></pre>
    </article>
  </div>
</section>
<section class="pair">
  <h2>Recovery is one command.</h2>
  <div class="rec">
    <div><h3>A crash</h3><pre class="term"><span class="p">$</span> <b>lf</b> resume --all</pre><p>Every conversation a restart interrupted comes back.</p></div>
    <div><h3>A spent login</h3><pre class="term"><span class="p">$</span> <b>lf</b> account</pre><p>Runs move to your next login when one runs out.</p></div>
    <div><h3>A stuck model</h3><pre class="term"><span class="p">$</span> <b>lf</b> task run ETU-14 -a codex</pre><p>Same Task, same branch, a different agent.</p></div>
    <div><h3>A busy laptop</h3><pre class="term"><span class="p">$</span> <b>lf</b> --machine mini task run ETU-14</pre><p>Run it on another of your machines over ssh.</p></div>
  </div>
</section>
<section class="pair"><h2>With your team, too.</h2><p class="lede">The Tasks are shared through Linear or a Git ref. Comment on one and its agent reads it before its next phase.</p></section>
{teams}
{works}
<section class="belief"><p>Scale together.</p></section>"""


# ---------------------------------------------------------------- Take C

C = f"""<section class="home-hero tc">
  <p class="eyebrow">Take C · hand it to your agent</p>
  <h1>Ask your agent if you should use <code>lf</code>.</h1>
  <p class="sub">It’s the CLI agents use to launch subagents and get work done. The docs are written for your agent to read.</p>
  <div class="ask">
    <p class="lbl">Paste this into Claude Code, Codex, OpenCode or Pi</p>
    <div class="install-row"><pre tabindex="0" class="term">Read loopflow.studio/llms.txt. Should we use it here? If yes, try it on one small change.</pre><button type="button" data-copy="Read loopflow.studio/llms.txt. Should we use it here? If yes, try it on one small change." class="copy">Copy</button></div>
  </div>
</section>
<section class="tc-split">
  <article class="for-you">
    <p class="eyebrow">What you get</p>
    <h2>Your agent stops doing everything in one conversation.</h2>
    <p>It files a Task for each piece, starts a subagent on each one in its own worktree, and comes back with pull requests. When one stops partway, another picks it up from the branch.</p>
    <p>You get asked at the points you chose: the design, the demo, both or neither.</p>
  </article>
  <article class="for-agent">
    <p class="eyebrow">What your agent reads</p>
<pre class="md"><b># Loopflow</b>
lf launches Claude Code, Codex, OpenCode or Pi
as subagents, each in its own git worktree,
and records what they did.

<b>## Try it</b>
lf -b implement "fix the flaky retry test"
lf pr publish

<b>## Needs</b>
macOS or Linux · git · a signed-in agent CLI
gh for pull requests · Linear optional

<b>## Touches</b>
~/.lf · .lf/ · scratch/ · worktrees beside the repo
your own provider login · no hosted server

<b>## Not built yet</b>
Asana and Notion planning</pre>
  </article>
</section>
<section class="pair">
  <h2>What it’s likely to say.</h2>
  <div class="verdict"><p>“Yes. It’s a CLI that launches me and other agents as subagents, each on its own branch, so a failure doesn’t lose the work. It works with the Claude Code you already have. The smallest test is one background run on the flaky retry test; it takes a few minutes and leaves a PR you can close. Worth trying.”</p></div>
</section>
<section class="pair"><h2>Then bring your team.</h2><p class="lede">Tasks your agent files are a plan your team shares, through Linear or a Git ref.</p></section>
{teams}
{works}
<section class="belief"><p>Scale together.</p></section>"""


INDEX = """<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Homepage takes</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Instrument+Serif&family=Inter:wght@400;600&family=JetBrains+Mono&display=swap">
<style>body{font:16px/1.6 Inter,sans-serif;max-width:760px;margin:60px auto;padding:0 20px;color:#1a1a1a;background:#fff}h1{font:400 44px 'Instrument Serif',serif;color:#722f37;margin:0 0 8px}a{color:#9B1A4A}li{margin:18px 0}small{display:block;color:#555}code{font-family:'JetBrains Mono',monospace;font-size:.9em}</style></head><body>
<h1>Three takes</h1><p>“The CLI your agents use to get work done.” Each keeps the teams section from the last pass.</p><ul>
<li><a href="take-a.html">A · Transcript-led</a><small>Headline is the line itself. The hero picture is one Claude Code turn: split, a subagent fails, the next one finishes. Then built-in vs <code>lf</code> subagents.</small></li>
<li><a href="take-b.html">B · Contract-led</a><small>“Break the work into phases. Let any agent finish it.” The picture is the workflow with artifacts between phases; recovery gets four one-command cards.</small></li>
<li><a href="take-c.html">C · Hand it to your agent</a><small>“Ask your agent if you should use lf.” The CTA is a prompt to paste; the page shows what the agent reads and the verdict it should reach. Ties the honest-docs work to the homepage.</small></li>
<li><a href="current.html">Current homepage</a></li></ul></body></html>"""

for slug, name, title, body in [
    ("take-a", "A · transcript-led", "the CLI your agents use to get work done", A),
    ("take-b", "B · contract-led", "break the work into phases", B),
    ("take-c", "C · hand it to your agent", "ask your agent", C),
]:
    (HERE / f"{slug}.html").write_text(page(name, title, body))
(HERE / "index.html").write_text(INDEX)
cur = current.replace('href="/static/', 'href="../../website/static/').replace('src="/static/', 'src="../../website/static/')
(HERE / "current.html").write_text(cur)


# ---------------------------------------------------------------- Split: home + /cli

clihero = re.search(r'<section class="pair clihero split">.*?</section>', current, re.S).group(0)
home_main = current.split('<main id="main-content" class="home">')[1].split("</main>")[0]
CLI_BAND = """<section class="cli-band">
  <p class="eyebrow">Underneath</p>
  <h2><code>lf</code>: the CLI your agents use to get work done.</h2>
  <p>Tell your coding agent to use it. It hands the work to background runs on any model, each in its own worktree, and stays free to talk while they work.</p>
  <p class="more"><a href="cli.html">The CLI →</a></p>
</section>"""
HOME_SPLIT = home_main.replace(clihero, CLI_BAND)



WHAT = """<section class="pair">
  <h2>What <code>lf</code> does.</h2>
<pre tabindex="0" class="term what"><span class="p">$</span> <b>lf</b> : "fix the failing test"          <span class="c"># launch an agent with a prompt</span>
<span class="p">$</span> <b>lf</b> -a codex : "review this branch"   <span class="c"># any agent, any model</span>
<span class="p">$</span> <b>lf</b> skill design "add CSV export"     <span class="c"># a saved prompt: a Skill</span>
<span class="p">$</span> <b>lf</b> graph pursue                      <span class="c"># Skills in sequence: a Graph</span>
<span class="p">$</span> <b>lf</b> graph customize pursue            <span class="c"># copy it into your repo</span>
<span class="p">$</span> <b>lf</b> -b graph pursue                   <span class="c"># the same, headless</span>
<span class="p">$</span> <b>lf</b> resume                            <span class="c"># the last conversation here</span>
<span class="p">$</span> <b>lf</b> session list                      <span class="c"># every conversation, any agent</span>
<span class="p">$</span> <b>lf</b> launch "add CSV export"           <span class="c"># a Task: worktree, branch, PR, running</span>
<span class="p">$</span> <b>lf</b> task wait EXP-12                   <span class="c"># block until it settles</span>
<span class="p">$</span> <b>lf</b> pr publish                        <span class="c"># a draft PR</span>
<span class="p">$</span> <b>lf</b> land                              <span class="c"># merge when checks pass</span>
<span class="p">$</span> <b>lf</b> account                           <span class="c"># logins and what's left on each</span>
<span class="p">$</span> <b>lf</b> usage --task EXP-12               <span class="c"># what that Task cost</span>
<span class="p">$</span> <b>lf</b> --machine mini launch EXP-12    <span class="c"># on another of your computers</span></pre>
      <p class="cap">The launcher needs no Task, no Wave and no Linear. MIT licensed.</p>
</section>"""

FIND = """<section class="pair find">
  <h2>Forgot where you left something? Ask.</h2>
  <p class="lede">Every Task your agent launches gets its own conversation, kept with no setup: it knows where the work is while runs do it, and it’s still there next week. Pick one up yourself with <code>lf resume --task ETU-12</code>, or ask your agent to find it by what it was about.</p>
<div class="tx small" role="img" aria-label="Asking your agent to find and open a Task you forgot about">
<p class="you">› I was working on streaming metrics somewhere but forgot where. Can you open that task for me?</p>
<p class="run"><b>lf</b> task list</p>
<p class="out dim">  ETU-14  rent CPU pods for templates with no GPU  implement · 2/4</p>
<p class="out dim">  ETU-15  backfill pod kind for old runs           ship · landed</p>
<p class="out">  ETU-12  stream metrics from rented machines      design · waiting on you</p>
<p class="say">That’s ETU-12, “stream metrics from rented machines”. Its design is waiting on you. Opening it.</p>
<p class="run"><b>lf</b> open --task ETU-12</p>
<p class="out dim">  opened in tmux window 4</p>
</div>
</section>"""

FOUR = """<section class="pair four">
  <h2>The best of four kinds of agent tool, in one CLI.</h2>
  <p class="lede">Memory layers, agent messaging, task trackers and orchestrators each solve part of getting agents to build on each other’s work. <code>lf</code> takes the part each one gets right, and keeps all of it in your repository.</p>
  <div class="grid4">
    <div><p class="eyebrow">From memory layers</p><h3>Next week’s agent knows what this week’s decided.</h3><p class="how">The decision is in the design doc on the branch, where the next agent reads it. So can you.</p></div>
    <div><p class="eyebrow">From agent messaging</p><h3>Change your mind without starting over.</h3><p class="how"><code>lf comment ETU-14 "smallest pod by default"</code>, and the run picks it up before its next step.</p></div>
    <div><p class="eyebrow">From task trackers</p><h3>Ask for ten things, get ten PRs.</h3><p class="how">Each one small enough to review on its own, and no two agents ever on the same one.</p></div>
    <div><p class="eyebrow">From orchestrators</p><h3>Use every model you pay for.</h3><p class="how">Claude designs it, Codex reviews it, each in its own worktree, all at once.</p></div>
  </div>
  <p class="eyebrow only-h">And what only <code>lf</code> does</p>
  <div class="grid3">
    <div><h3>Skill Graphs.</h3><p>Phases as files in your repository. Each says what to leave behind, so any agent, on any provider, continues from the last one’s result instead of its memory.</p></div>
    <div><h3>A conversation per Task.</h3><p>Every Task your agent launches keeps its own conversation: resume it next week, or have your agent open it in your tmux, cmux or Herdr.</p></div>
    <div><h3>Your logins, all of them.</h3><p>Runs use your own Claude, Codex, OpenCode or Pi subscriptions, several per provider, and move to the next login when one runs out.</p></div>
  </div>
</section>"""

CONTEXT = """<section class="aside">
  <p><b>And it knows where it’s called from.</b> In tmux, <code>lf open</code> uses the next window; in cmux or Herdr, a new workspace. Inside a Task’s worktree, you don’t name the Task. From your agent’s shell, every run is recorded as that conversation’s.</p>
</section>"""

# The old /cli page's list (before #1637), current nouns, the parts still true.
USE_LIST = """<section class="pair">
  <h2>Use as much as you want.</h2>
  <dl class="uses">
    <div><dt>Launcher</dt><dd>Use <code>lf</code> anywhere you would start Claude Code, Codex, OpenCode or Pi, so every agent starts the same way.</dd></div>
    <div><dt>Sessions</dt><dd>Every conversation it starts is kept, whichever agent ran it. <code>lf resume</code> picks the last one back up; <code>--all</code> brings back everything a crash interrupted.</dd></div>
    <div><dt>Skills</dt><dd>A prompt saved as a Markdown file. <code>lf skill design "add CSV export"</code></dd></div>
    <div><dt>Graphs</dt><dd>Skills in sequence, with commits between. Built-ins for software engineering, or your own in <code>.lf/graphs/</code>.</dd></div>
    <div><dt>Interactive or headless</dt><dd>Any Skill or Graph as a conversation or in the background with <code>-b</code>. Each step can use a different agent.</dd></div>
    <div><dt>Worktrees and PRs</dt><dd>A Task is one worktree, one branch, one pull request. <code>lf pr publish</code>, <code>lf land</code>.</dd></div>
    <div><dt>Accounts and usage</dt><dd>Several logins per provider; work moves to the next when one runs out. <code>lf usage</code> shows what it cost.</dd></div>
    <div><dt>Machines</dt><dd><code>--machine</code> runs any of it on another of your computers over ssh.</dd></div>
  </dl>
  <p class="cap">The launcher needs no Task, no Wave and no Linear. Graphs run without Tasks; a Task can run any Graph. MIT licensed.</p>
</section>"""

CLI = f"""<section class="home-hero ta">
  <h1>The CLI your agents use to get work done.</h1>
  <p class="sub">Tell your coding agent to use <code>lf</code>. It hands the work to background runs, each on its own branch, and stays in the conversation with you, free to answer, change course, or stop a run mid-step.</p>
  <div class="tell"><p class="lbl">Give your agent the skill</p>{tell_row()}<p class="lbl">or install the CLI</p>{install}</div>
</section>
<section class="ta-shot">{TRANSCRIPT}
  <p class="cap">One conversation in Claude Code. It never did the work itself, so it was free when you changed your mind, and nothing was lost when a run died.</p>
</section>
""" + A.split('<section class="pair">', 1)[1].split('<section class="pair">\n  <h2>And when', 1)[0].join(['<section class="pair">', '']) + WHAT + """
<section class="pair"><h2>Want to see it all at once?</h2><p class="lede">Loopflow for Mac shows every Task, run and conversation <code>lf</code> knows about, built on the same commands. <a class="u" href="home-split.html">Loopflow for Mac →</a></p></section>
""" + works

CLI = CLI.replace('</section>\n<section class="pair">\n  <h2>Every phase has a contract', '</section>\n' + FIND + '\n' + CONTEXT + '\n<section class="pair">\n  <h2>Every phase has a contract', 1)

# Graphs lead: move the contract section to just after the transcript.
_c0 = CLI.index('<section class="pair">\n  <h2>Every phase has a contract')
_c1 = CLI.index('</section>', _c0) + len('</section>')
_contract = CLI[_c0:_c1]
CLI = CLI[:_c0] + CLI[_c1:]
_t = CLI.index('</section>', CLI.index('class="ta-shot"')) + len('</section>')
CLI = CLI[:_t] + '\n' + _contract + '\n' + FOUR + CLI[_t:]

for slug, name, title, body in [
    ("home-split", "Split · homepage", "how you build software together", HOME_SPLIT),
    ("cli", "Split · /cli", "the CLI your agents use to get work done", CLI),
]:
    html = page(name, title, body)
    html = html.replace('<a href="/docs">Docs</a>', '<a href="cli.html">CLI</a><a href="/docs">Docs</a>')
    (HERE / f"{slug}.html").write_text(html)
(HERE / "index.html").write_text(INDEX.replace('<li><a href="current.html">', '<li><a href="home-split.html">Split · homepage</a><small>Current homepage; its <code>lf</code> section becomes a short band linking to /cli.</small></li><li><a href="cli.html">Split · /cli</a><small>Take A’s frame and contract section, then Sessions and runs, then the old /cli page’s “use as much as you want” list.</small></li><li><a href="current.html">'))


# ---------------------------------------------------------------- /cli around Work, Context, Orchestration, Sessions

CATS = [
    ("Work", "Say what you want. It becomes Tasks that run without you.",
     "Your agent turns each piece into a Task with its own worktree, branch and pull request, so ten requests come back as ten PRs you can review one at a time.",
     'lf task create --title "rent CPU pods"\nlf -b task run ETU-14'),
    ("Context", "Every agent starts where the last one left off.",
     "Each launch carries the Wave’s memory, the Task’s notes in <code>scratch/</code>, the docs you name and any new comments, and gets them back after compaction.",
     'lf comment ETU-14 "smallest pod by default"\nlf gate --docs docs/runpod.md'),
    ("Orchestration", "Work finishes while you’re doing something else.",
     "Runs go to the agent and machine you pick, Graphs carry them step to step, Waves keep their Tasks moving, and landing repairs CI until the PR merges. Get as much as possible off the main thread: plans into code, code into verified code.",
     'lf --machine mini -b -a codex graph pursue\nlf land --wait-and-fix'),
    ("Sessions", "You never lose a conversation.",
     "Every conversation outlives its terminal, and new work starts on whichever of your logins has room. Close the window and come back next week; the conversation and what it knows are still there.",
     'lf resume --task ETU-14'),
]


def _cat_card(name, benefit, how, cmds):
    cmd_html = "\n".join(f'<span class="p">$</span> <b>lf</b>{c[2:]}' for c in cmds.split("\n"))
    return (f'<div class="cat"><p class="eyebrow">{name}</p><h3>{benefit}</h3><p>{how}</p>'
            f'<pre class="term">{cmd_html}</pre></div>')


CATS_SECTION = ('<section class="pair cats"><h2>Everything your agents need to build on each other’s work.</h2>'
                '<div class="grid-cats">' + "".join(_cat_card(*c) for c in CATS) + "</div></section>")

_c0 = CLI.index('<section class="pair">\n  <h2>Every phase has a contract')
_c1 = CLI.index('</section>', _c0) + len('</section>')
GRAPHS_SECTION = CLI[_c0:_c1].replace("<h2>Every phase has a contract.</h2>", "<h2>Skill Graphs hold it together.</h2>")

SKILL_GRAPHS = """<section class="pair">
  <h2>Skill Graphs: write down how you work.</h2>
  <div class="halves">
    <article>
      <p>Skills are Markdown and Graphs are YAML, files in your repository reviewed like code.</p>
      <p>Loopflow ships reasonable workflows for software engineering, and they are templates. Copy one with <code>lf graph customize pursue</code>, change it, or write down the way you work and share it.</p>
<pre tabindex="0" class="term"><span class="p">$</span> <b>lf</b> audit                 <span class="c"># run one Skill</span>
<span class="p">$</span> <b>lf</b> -b graph ship-api     <span class="c"># run the Graph headless</span></pre>
      <p class="more"><a href="/docs/authoring">Write a Skill ↗</a><a href="/docs/authoring#graphs">Graphs ↗</a></p>
    </article>
    <article>
<pre tabindex="0" class="term"><span class="c">&lt;!-- .lf/skills/audit.md --&gt;</span>
Audit auth changes on this branch. Fix missing
validation and untested paths.</pre>
<pre tabindex="0" class="term g2"><span class="c"># .lf/graphs/ship-api.yaml</span>
- design      <span class="c"># leaves scratch/&lt;task&gt;.md</span>
- implement   <span class="c"># leaves commits on the branch</span>
- audit
- gate        <span class="c"># leaves passing checks</span></pre>
    </article>
  </div>
</section>"""

CLI_V2 = f"""<section class="home-hero ta">
  <h1>The CLI your agents use to get work done.</h1>
  <p class="sub"><code>lf</code> is the command you type <em>and</em> the CLI your coding agent uses. What you ask for becomes Tasks that run in the background, every agent starts where the last one left off, and no conversation is ever lost.</p>
  <div class="tell"><p class="lbl">Give your agent the skill</p>{tell_row()}<p class="lbl">or install the CLI</p>{install}</div>
</section>
{CATS_SECTION}
{SKILL_GRAPHS}
<section class="aside"><p><b>Use what you like and skip what you don’t.</b> The launcher needs no Task and no Linear: <code>lf : "fix the flaky retry test"</code> starts one agent, like starting it yourself.</p></section>
<section class="pair"><h2>Want to see it all at once?</h2><p class="lede">Loopflow for Mac shows every Task, run and conversation <code>lf</code> knows about, built on the same commands. <a class="u" href="home-split.html">Loopflow for Mac →</a></p></section>
{works}"""

html = page("/cli · four benefits", "the CLI your agents use to get work done", CLI_V2)
html = html.replace('<a href="/docs">Docs</a>', '<a href="cli.html">CLI</a><a href="/docs">Docs</a>')
(HERE / "cli.html").write_text(html)

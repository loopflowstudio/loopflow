# Loopflow Studio front page

Jack Heart requested this worktree to refine loopflow.studio around one company
and three efforts:

| Effort | Purpose |
| --- | --- |
| Loopflow | Software orchestration |
| Fantasia | Superhuman Magic: The Gathering bots |
| Etude | Experiment orchestration |

## Accepted direction

Loopflow Studio is the company. The present Magic/manabot repository is intended
to become Fantasia; a separate Etude repository is intended for the experiment
service. These are naming/product decisions, not evidence that repository renames
or extraction have already occurred.

Magic is pursued for the joy of it and as a demonstration of pure research
pursued with craft. It does not need to be framed as a commercial customer workload.

Etude is a hosted experiment service combining execution through Loopflow with
a database and UX for organizing research. Etude is proprietary; its customizable research resources are not open source.
It connects research questions, experiments, training runs/jobs, reports,
findings and recommendations. General infrastructure includes budgets, launch
parameterization, hardware allocation, supervision and provenance.

Etude's default learning ontology and research flows/skills are general, not
RL-specific. They demonstrate a useful starting practice; customers are expected
to customize them with agents through version-controlled, reviewable definitions.

## Scope for this session

Refine the front page's hierarchy, copy and presentation with Jack Heart.
Express the three efforts clearly under Loopflow Studio, distinguishing product
ambitions from shipped capabilities. Read website/README.md and the current
homepage implementation before proposing changes. This worktree is prepared for
Jack Heart's interactive work; no implementation worker or deployment was started.

## Etude's purpose

Jack Heart describes the essence of Etude as: do science within capitalist
enterprises. While pursuing revenue, follow the scientific method.

The service should help commercial teams make explicit hypotheses, run informative
experiments, inspect evidence, and revise their beliefs and decisions. Revenue is
a legitimate objective; scientific discipline guides its pursuit. This purpose
extends beyond ML research and should inform Etude's front-page positioning.

## Loopflow positioning carried in from another Session (2026-10-08)

Jack Heart pasted a transcript from a product-positioning conversation into this
Task's conversation. Three lines in it read as his own; the replies around them
are that Session's agent and are drafts, not approvals.

- "orchestra metaphor works as a top level phrase. this needs to be connect more
  to the 'the work'": the podium/orchestra line stays as top-level positioning;
  below it, copy talks about the work.
- "so a software engineering CLI for your agents"
- "i think one thing i really want to emphasize is the idea of loopflow bieng
  software used by agents as much or more than for using agents"

The October 7 approved hero ("A command line for great software engineering")
predates these. Whether the front page's Loopflow line changes is open.

## Found while reading (2026-10-08)

- `github.com/loopflowstudio/etude` is today the Magic repository; its README
  titles it "Etude Fantasia". A link labelled Etude would land on Magic work.
- `/loopflow`, `/products`, `/team` and `/story` exist as redirects; `/loopflow`
  goes to `/download`.
- The site footer already says "Built by Loopflow Studio".
- wave/growth/MEMORY.md says the site does not state the revenue plan. A hosted
  Etude service on the front page is a commercial statement; unresolved.

Jack Heart then gave the line for this page in this conversation, word for word:
"Loopflow is a software engineering CLI for your agent". Singular "agent" is his.

## Structure decided by Jack Heart (2026-10-08, this conversation)

- "B - product stays at the root. loopflow.studio should basically implicitly
  mean /loopflow".
- "we start to have a little bit of a header and then smaller pages for etude
  and fantasia and the company page says that we have these three things, we are
  building this to sell to people, we build this tool to help us do it, we are
  doing this research to do it well".

Read as: Etude is what is sold, Loopflow is the tool for building it, Fantasia
is the research for doing it well. This puts a revenue statement on the site,
which supersedes the growth memory line that the site does not state the plan;
Loopflow itself stays free. Routes built: `/` product, `/studio`, `/fantasia`,
`/etude`, with a studio bar above each. All copy beyond Jack's quoted lines is
draft.

Jack Heart on the Etude page: "etude's page can be super short. We are building this - no links, no pics, info anything".

## Fantasia page direction from Jack Heart (2026-10-08)

Jack cut "They do not yet." and "Research pursued for the joy of it, and to show
what research done with craft looks like." He asked for more of the Fantasia
repository's own language, for the page to be "a little bit more explicit about
using AI mtg as lever to drive the development of loopflow and etude", and for a
link to the Fantasia GitHub. The repository is public at
github.com/loopflowstudio/fantasia; the rename from etude has happened. Its
README supplied "AI-assisted Pro Tour testing house" and "serious practice,
visible disagreement, rapid experiments, and the fun of discovering something
together." No line in that repository is quoted as Jack's; the lever wording on
the page is a draft.

Jack Heart set the order everywhere: Loopflow, Etude, Fantasia.

Jack Heart replaced the homepage headline "A command line for great software engineering." with "A software engineering CLI for your agents" (plural). The /studio Loopflow row was aligned to the plural.

## Homepage restructure from Jack Heart (2026-10-08, later in this conversation)

Jack's directions, in order:

- The subline "needs to be balanced against the other perspective": a sentence
  on what a software engineering CLI means, "if you're asking your agents to
  build software, your agent can do it basically just by using this CLI To break
  its work down into concrete steps in traceable and reconnectable ways".
- Two main sections. One: user agents, surface terminal, offering "craft of
  software, api around all the parts of the process (planning, designing,
  prototping, implementing, testing, optimizing, researching, landing, etc)".
  Two: user human, surface "OS X native, or alternative a user of herdr/cmux",
  offering "peace of mind, focus, flow, from having exactly what needs your
  attention on screen by havung the UX map directly to what matters to you, and
  not be path dependent and meandering chats".
- Section titles "For your agents" and "For You". "Give your agents the tools to
  do things the right way." and "Stay in flow while you work with agents." are
  his lines and sit under them.
- Organization copy: "long lived sessions around workstreams and results, not
  meandering chats" and "isolated jobs for routine process and autonomous
  development". "Our organization system is not special and shoudlnt be sold as
  such. Its just minimally simple and flexible - you can have as many waves sas
  you want. and it just works with everything".
- "Kill the To empower section and combine the previous 3 into 2".
- No double header: one top bar structured like cmux's and herdr's (brand, flat
  links, one primary button), with a Company menu holding Loopflow Studio, Etude
  and Fantasia, and "an about that is just my name or something basically".

Built as directed. Still draft, not his wording: both section introductions, the
three part titles under For your agents, the two "kinds" paragraphs, the "How it
is organized" line, and the About page's one line. "It just works with
everything" is not on the page; no claim was drafted for it. The four rows
(Wave, Project, Task, Workflow) remain inside For you; whether they stay on the
homepage is open. Only the structure of the cmux and herdr bars was studied; no
code or copy was taken.

## Merge and variants requested by Jack Heart (2026-10-08)

Jack asked for the hero's command list to replace the right-hand box in For
your agents, the headline and its agent sentence to merge into that section,
the podium and orchestra language to merge into For you, Task content to move
into the first section and Wave content into the second, and both sections to
grow. Built: the page opens with For your agents (headline, his claim, Flow
diagram beside the command list, ownership strip, three parts, Task and
Workflow rows), then For you (podium line, Mac capture, the two kinds, Wave and
Project rows), then install.

He then asked for three new versions branching from this work that "explore
alternative ways to be inspired by Herdr or cmux's website to counterposition
and use some of the structures that they use". They are standalone files in
`scratch/prototypes/`; the site itself is unchanged by them.

Jack Heart then asked for a mix of "20% A 40%B 40% C", made three different
ways. `scratch/prototypes/blend-a.html`, `blend-b.html` and `blend-c.html` are
assembled from the three earlier prototypes' own sections: B's install-in-hero
and numbered claims, C's two-pane surface and where-it-sits stack, and A's FAQ,
plain list or name-as-headline opening. They differ in what opens the page and
where the stack sits.

## Topline candidate (2026-10-08)

Jack Heart proposed a topline: "Build software by conducting agents in harmony"? Asked as a question, not settled.
Tried in `scratch/prototypes/blend-c.html` under the name; blends A and B keep "A software engineering CLI for your agents." for comparison.

## Blend A chosen (2026-10-08)

Jack Heart: "lets do A. cut the Where it sits section". Blend A is the structure; the three-row stack is removed from it.
The "Your machines / Your methods" ownership strip lived inside that section and went with it.

## Lead with the desktop app (2026-10-08)

Jack Heart, after blend A was ported: "i think ive changed my mind. lets go back to leading with the desktop app. when sellign the vision of loopflow, we should show the app in its fullness. then we can lean into the terminal stuff later on."
Order becomes: hero, the Mac capture at full width, For you, For your agents (terminal), FAQ, install.

Jack Heart on the FAQ's first answer ("No. lf is a command…"): "This is sort of wrong, Looplfow Desktop is a terminal." Answer rewritten to say the Mac app is one and `lf` also runs in other terminals.

## Two pages: anyone, and engineers (2026-10-08)

Jack Heart, new direction for the main page:
"Loopflow lets anyone craft software. You need: a coding agent (claude, codex, or opencode); a github. We'll get you building whatever software you want like a software engineer, by guiding you through workflows that get the most out of you and ensure high quality software, not slop."

Second page:
"Loopflow is the CLI for software engineering. It's a big claim, but let's own. Let's make the case that our data model and APIs is the right one to encode what matters in high quality software development and avoids slop. For techincal people, we are NOT competing on Loopflow Desktop. We are saying we work wherever you work. Use as much as you want. Change whatever you want."

Built: `/` carries the first, `/cli` the second (it used to redirect to docs). The terminal claims, the command list and the Task / Workflow / Wave / Project rows moved to `/cli`.
Open: `/cli` does not yet argue why this data model is the right one; it shows the model. Section headings there are draft.

## Hero video (2026-10-09)

Jack Heart: the Mac app / anyone page "really needs a video of a flow from initial just first usage -- starts in repo/session, to then eventually like some waves and tasks and then eventually like some sort of complex multi-split set up showing task flows". He asked Claude to record it, on "a legified version of the kata repo", using "the best you can find in todays app"; length "might be like 10seconds, 3 seconds per stage or seomthing. or it oculdbe up to 60sec".

What exists in the app today (read from source, not yet seen running): no onboarding flow; Go > Open Repo (⇧⌘O); New Session with a skill picker; Waves → Tasks → Sessions navigator; pane splits (⌘D, ⇧⌘D) and worktree-level splits; a Task workflow graph above the terminals. `LF_HOME` isolates state; `--repo` picks the repository.
Kata today: Waves core, dj, ear, release, teacher, theory (scores abandoned), 8 tasks.
Blocked: the cmux Computer Use helper is not running, so nothing has been recorded.

## /cli page, cmux feel (2026-10-09)

Jack Heart on `/cli`: "Cut Ask your agents... Cut For Engineers. Cut the headers for the lf skill frame. Replace the list that follows with a list expanding on the cmd list that gradually explains: Align your agent sessions around core hierarchy of work: repo->wave->task; task=worktree=branch git workflows; skills and workflows to implement PRs; ... other cool shit and why you should use lf to launch your agent sessions in whatever terminal you like".
Then: "this is probably a little more on the cmux-y feel" and "in this world were tryign to get users of cmux to use lf inside lf, not pushing Loopflow Desktop". (Read as: lf inside cmux.)
Built: headline, install command, the command list, one plain `- Name: detail` list, "It works wherever you work." as a second list, install without the Mac download. The Task / Workflow / Wave / Project rows are gone; the first list carries them.

On the video: "legified" meant "made more legible, optimized for the audience rather than the author". So the recording uses a copy of kata whose Wave and Task names read well to a stranger, not kata as it stands.

Jack Heart, next pass on `/cli`: cut the Checks item; "move sessions to the top and make it make sense there". His copy, which he called "like 85% there":
"lf makes it easy to define and share workflows around agents. lf's most basic function is an agent launcher and session manager. lf can be used anywhere you would launch an agent. for example, you can resume a codex session in claude by using lf. lf is also a project manager. lf organizes your work into waves (long-lived streams) and tasks (concrete outcomes, i.e. PRs). it encodes all the stages that a task goes through in order to turn into a shipped and validated PR, and allows you to customize on a project-by-project basis how and when to get involved with the work being done in those areas. lf also provides a programmable desktop UX optimized around waves and tasks. lf is free, open-source, and can be used inside any terminal, or alongside tools like cmux and herdr."
Applied as the page's lede and list items, lightly tightened.
Then: "oh needs a section on flows afte the project manager part" and "cut the programmable desktop UX". `/cli` now has a Flows section with a real Flow file after the first list, and no desktop line.

Jack Heart's Flows copy for `/cli`: "lf allows you to run flows. flows are (looping) sequences of skills, each step of which can be configured to be interactive or autonomous, run on a particular model, etc. lf provides lots of reasonable built-in flows that you can use to do software engineering, but you are encouraged to build your own and share them with others. loopflow aspires to help you focus on the stages that matter most to you." Applied; the last sentence became the section heading.
Jack Heart: "lf is the CLI for defining workflows with coding agents." Placed as the `/cli` lede under the headline; whether it replaces the headline is open.

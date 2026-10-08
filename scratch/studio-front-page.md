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

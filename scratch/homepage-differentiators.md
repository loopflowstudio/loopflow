# Homepage differentiators

Rework loopflow.studio's homepage (`website/home.html`, `website/content.yaml`)
so a visitor comparing agent orchestrators (Conductor, Orca, Paradev, Paseo,
T3 Code, Happy, HumanLayer, Emdash, Maestro) sees what Loopflow does that they
don't. The table stakes they all lead with should be visible too.

Page: [scratch/explore.html](explore.html)

## What makes Loopflow different

Jack Heart, 2026-11-07:

> i think its:
>
> - loopflow has built in graphs and loops that allow you to configure how the flow of your work
> - loopflow organizes agents around tasks, and separates the conversation from the work in helpful ways
> - you can use loopflow from inside your favorite terminal multiplexer, like rex

Jack Heart, 2026-11-07, on the third point:

> 3. You can use Loopflow from inside your favorite terminal multiplexer, like Rex. *OR* use our desktop and mobile apps? im not sure. is this mied messgagin?

Open: page question 6.

Jack Heart, 2026-11-07, on closest matches:

> i think warp software factories is probably teh closest spiritual match (along with human alyer)

> but the product shape is more like the agent IDEs

> because we are a open-source single dev proejct

Jack Heart, 2026-11-07, on taking Warp's high-level framing:

> is there a way to take more of hte high level messaging and problem-we're-solving from warp factories

> its not just Measurement > vibes. .. its just ... craft and intentionality at scale

Warp's page, in order: a named bottleneck ("Solving the code review
bottleneck"), a worldview line ("Measurement > Vibes"), a scope claim ("Beyond
CI/CD to automating the whole SDLC"), an enemy ("Control your coding agent
chaos"), build-versus-buy ("The factory infrastructure you'd build yourself"),
and a number ("most orgs start around 20-30% of PRs fully automated").

Open: page vision 4 and question 1e.

Jack Heart, 2026-11-07, on the problem line:

> Shipping tens of thousands of lines is the easy part.
>
> Shipping is now the arbitrarily large volumes of code is the easy part. The hard part is scaling your attention and vision.

The page reads the second line as "Shipping arbitrarily large volumes of code
is now the easy part. The hard part is scaling your attention and vision."
That reading is unconfirmed (page question 9).

> something like that. Maybe not quite

Round 2, 2026-11-07:

> can i see three full new prototypes of the full homepage

The page now holds three full homepages: P1 Craft at scale, P2 Follow one
Task, P3 Factory in your terminal. Each one uses a different reading of the
problem line, so the line can be judged in context.

> i feel also the github readme mgiht be an easier place to add more about the basics of that loopflow is doing the same as everyone else

> A1 works

"A1" is the first of the refine options for the problem line in the
conversation: "Shipping code is no longer the hard part. Scaling your
attention and vision is." All three prototypes now use it.

README sketch: the README already has a "What else it does" table. Rather
than add a second table, which would put the same information in two places,
reorder that one so the rows every orchestrator lists come first, and add
the missing ones:

| You want to… | |
|---|---|
| run agents in parallel, each in its own worktree and branch | `lf task create "…" --run` |
| run on Claude Code, Codex, OpenCode or Pi, on your existing subscriptions | `lf -a codex : "…"` |
| use every Claude and Codex login you pay for, routed to the one with room | `lf account` |
| keep work already in progress (another tool's worktrees, a dirty checkout) | `lf task create --worktree . --title "…"` |
| have your own agent drive all of it | `npx skills add loopflowstudio/loopflow --skill loopflow -g` |
| run on your own machine over SSH | `lf --machine mini -b task run <ID>` |
| land when checks pass, with CI failures repaired for you | `lf land --wait-and-fix` |
| …the remaining existing rows | |

Also move "There is no hosted service: conversations stay on your machine"
from "What it touches" into the README's opening paragraph.

Warp Factories (warp.dev/factories, read 2026-11-07): "Measurement > Vibes";
"open infrastructure for teams to build measurable, self-improving software
factories"; `factory.yaml` declares agents, triggers, stages with approval
gates ("merge is always yours"), scorers and benchmarks. It runs in Warp's
cloud or your own VPC, and is priced per agent run.

Open: page question 7.

## Table stakes every competitor leads with, which Loopflow has but buries

- Parallel agents, each in its own worktree
- Runs on the subscriptions you already pay for, routed to the login with room (`lf account`)
- No hosted service: conversations stay on your machine, only the plan travels
- Your own agent can drive it (`npx skills add … --skill loopflow`)
- Remote machines over SSH, and rented machines (`--rent`) that are returned after each Task
- `lf land --wait-and-fix` repairs CI before merging

Things competitors have that Loopflow lacks: a phone app, Windows, built-in
schedules or event triggers, a long list of supported agents. Don't claim them.

## Conductor timing

Meta acquired Conductor on 2026-11-04. Conductor Cloud goes read-only on
2026-11-13 and shuts down on 2026-11-20. The Mac app keeps working until early
2027. Conductor reports more than 100,000 users. Orca publishes a "Migrate
from Conductor" guide. Paradev imports `~/conductor/workspaces`.

`lf task create --worktree <path> --title …` already adopts an existing
checkout, keeping its branch and its dirty and untracked files. Nothing
converts `conductor.json` scripts.

## Open

The page carries the numbered questions. No direction has been chosen.

Jack Heart, 2026-11-07, on P3's contrast heading ("Somewhere between an agent IDE and a cloud factory."):

> This shoudl be like "An IDE and a factory in one"

P3 now reads "An IDE and a factory in one."

Jack Heart, 2026-11-07, on P3's "Built with itself." section:

> skip it

Removed from P3.

Jack Heart, 2026-11-07:

> overall though i like 3 the best. but i think probably keep more of the personal voice from A

Read "A" as P1. Added P4: P3's structure, with body copy taken from the existing homepage lines that P1 kept.

Jack Heart, 2026-11-07, on "Your agent can drive it.":

> yeah i have thought abou tthis part being the lead. I like it

The real-site draft now opens with it: the hero shot shows an agent splitting work into lf Tasks, and the first section after the hero is "Your agent can drive it."

Jack Heart, 2026-11-07, on the "An IDE and a factory in one." section (its two lists and the "And from neither: nothing hosted…" line):

> i think cut this

Cut. The scaling paragraph and command card that followed it stay, under "Across your machines and your subscriptions."

Jack Heart, 2026-11-07, on the big command card (lf wt create … lf --machine mini monitor):

> and i feel like this … tells the cleanest story. as it is right now things are a little bit too small to ever get a big jum

The card now has its own full-width section right after "Your agent can drive it", at a larger size, under the existing "lf: the workflow CLI for shipping software with coding agents." heading and lede.

Jack Heart, 2026-11-07, on merging, cutting or keeping the small sections (process, Waves, Tasks):

> merge i think

Merged into "Your process, as code.": the workflow paragraphs, one Wave sentence, the session-window line and one sentence on Tasks, next to the workflows card and one combined command block.

Discord (https://discord.gg/j57B5vyXNJ) added to the nav, the footer and the closing CTA.

Jack Heart, 2026-11-07:

> ok, now lets do github/README chanegs

README (its opening is shared with docs/index.md):
- "Your agent can drive it" moved up to sit right after the intro paragraph; the copy that came after section 1 is deleted.
- New paragraph: run as many Tasks at once as you have logins for, each in its own worktree; nothing hosted.
- "What else it does" reordered so the basics come first, with two new rows: parallel Tasks, and keeping work already in progress (`lf task create --worktree`).
- Discord added under Docs.

Jack Heart, 2026-11-07:

> is there nothing from the things that they all explain that we failed to explain

Added a homepage FAQ (free, agents, privacy, platforms, how many at once, review, when an agent needs you, bringing in existing work), and "macOS and Linux" next to the license line.

Jack Heart, 2026-11-07, on the FAQ:

> i espected just more content explaining like the features and how to use them

Added "What you can do with it.": twelve features, each with a one-line description and the command that does it, after "Your process, as code." The FAQ is still on the page.

Jack Heart, 2026-11-07:

> ok, lets update the homepage. you can make it 50% longer but no more

> this isnt going to make the live website, so feel free to make it aspirational, along with a task list to make it true

Aspirational version at 1.50x the original height; the claims that are not yet true are listed in scratch/homepage-tasks.md.

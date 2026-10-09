# anyone.html: notes

Prototype of loopflow.studio's main page for Kim and Scott: curious, not
engineers, with an app they want to build. Jack Heart set the audience, the
hero copy and the two themes (minimal dependencies, maximal quality), then
added: build the page around the planned video, subtract the CLI audience and
see what is left, and join /cli in vibe.

## Section order

1. Hero. Jack's headline, "You need", his paragraph (grammar tightened),
   Download for Mac, cost line.
2. The app in full. The real capture in a slot labelled as a still, then the
   video's three beats: one conversation, Waves and Tasks, several at once.
   Beat 2 is marked "shown above" because that is what the still shows.
3. How one piece of work goes. Seven steps, each tagged You or Loopflow.
4. What you get. Plain `- Name: detail` list, benefit first.
5. What it needs. The whole dependency list, including the awkward parts.
6. Questions. Five.
7. Install. Download, cost line, and the one body link to /cli.

## Fears and where each is answered

| Doubt | Answered in |
|---|---|
| Will it make a mess I can't fix? | Step 4, "Work that cannot collide", Questions |
| What do I do all day? | The three "You" steps; Questions points back to them |
| What does it cost? | Hero cost line, "What it needs", Questions, Install |
| Do I need to know git? | Questions; "What it needs" names git and `gh` |
| Can I trust software I can't read? | Section 3 lede, Questions ("Loopflow cannot promise the software is right") |

## The walkthrough, mapped to the product

Source: `rust/loopflow/src/engine/builtins/task/workflow/feature.yaml` and the
Flows it names.

| Step on the page | Product |
|---|---|
| 1 Say what you want | A Task |
| 2 It writes a plan | edge `start → design`, Flow `task-design` (`kickoff`) |
| 3 You talk the plan over | node `design`, skill `review-design`; `design → design` is "another draft" |
| 4 It builds, off to the side | Flow `pursue`: `implement`; Task = own checkout, branch, one PR |
| 5 It goes back over its own work | `pursue`: `compress`, `loop: implement`, `pr-publish` |
| 6 You see it working | node `demo`, skill `demo`; `demo → demo` is "another pass" |
| 7 It ships | Flow `ship`: `gate`, then `pr land`; `lf ci watch` starts a repair on a failed landing |

The brief's order put "it checks itself" before "you see it working". In the
product the full gate (tests, static checks, docs) runs inside `ship`, after
the demo. The page follows the product: self-review at 5, gate at 7.

## What was left after subtracting the CLI

No commands, flags, YAML, or terminal blocks remain. What survived:

- The app as a place: project, sidebar of Waves → Tasks → Sessions, splits,
  the step diagram.
- The route a Task takes, described by who acts.
- Attention: waiting conversations first.
- Safety: separate copy per Task, one pull request per Task.
- Continuity: filed conversations, Wave memory.
- Cost and change visibility.
- A short, honest dependency list.

What fell away: launcher, Skills and Flows as files, headless runs, multiple
accounts, model choice per step, cmux and herdr, SSH, Linux. None of it is
missed by this reader. Skills and Flows survive only as "a way to start from
the list" and "the built-in route".

## What I took from /cli, and what I did differently

Took:
- One headline, then one concrete thing. /cli's command block becomes the app
  shown in full.
- The plain `- Name: detail` list where each item opens with what you get.
- A short second list. /cli's is "where it works"; here it is "what it needs".
- Install last, with the cost sentence beside it.
- No eyebrow slogans, no numbered marketing claims, nothing hidden: Linear,
  git and `gh` are on the page.

Different on purpose:
- Lists are set in the sans face at 16px, not 13.5px mono. Mono reads as
  "terminal" to this reader.
- A walkthrough section /cli does not have. An engineer knows how a change
  goes; Kim and Scott do not, and it is where "quality" becomes concrete.
- Questions survive. /cli has none; this reader's doubts are literal questions.
- Product nouns are defined in passing (session, Wave, Task, pull request).
  /cli assumes them.
- "Loopflow for Mac" eyebrow replaces "Loopflow is a software instrument":
  it says which product the page is about.

## Hero wording

Jack's paragraph is one sentence with a dangling "by guiding you". Split in
two, same meaning: "…the way a software engineer does. Loopflow guides you
through workflows that get the most out of you and keep the software high
quality, not slop." "A Mac" is not added to his "You need"; the button and the
needs section carry it.

## Claims and sources

| Claim | Source |
|---|---|
| Works with Claude Code, Codex, OpenCode, signed in, own cost | `docs/getting-started.md`, `README.md` |
| Free, open source, MIT | `LICENSE`, `README.md` |
| No account, no server, runs on your computer | `website/main.py` /download ("Runs on your computer. Nothing to register.") |
| macOS 15 or later | `swift/LoopflowMac/Info.plist` (`LSMinimumSystemVersion` 15.0), `swift/Package.swift` |
| git, GitHub CLI signed in | `docs/getting-started.md` "Before you install" |
| Tasks and Waves need Linear; a single conversation does not | `docs/getting-started.md`, `docs/waves.md` ("Tasks live in Linear") |
| Sessions are conversations inside the app | `docs/conducting.md` ("Each one is a terminal inside the app") |
| Sidebar of Waves, Tasks, Sessions; split panes; step diagram; start-from-a-list | Coordinator's list of what the app does today; `FlowGraphView.swift`, "Split right", "New Session", "Open Repo" strings in `swift/` |
| Still shows one Wave, its results, ten Tasks, four done | `website/static/wave-surface.png` |
| Plan, plan review, another draft | `feature.yaml`, `review-design.md` |
| Own copy, one pull request per Task | `website/content.yaml`, `docs/waves.md` |
| Simplifies, works until it holds | `pursue.yaml`, `docs/getting-started.md` ("implement until it holds") |
| Demo guided, another pass, closing is not approval | `demo.md` |
| Tests, checks, docs, then merge when checks pass; failed check can start a repair | `gate.md`, `ship.yaml`, `docs/getting-started.md` (`lf ci watch`), `content.yaml` |
| Second built-in route with demo only; chosen per project | `code.yaml`, `docs/authoring.md` |
| Waiting conversations listed first; roadmap marks waiting/blocked/clear; files changed; cost | `docs/conducting.md` "In the Mac app" |
| Wave notes of decisions and lessons, editable files | `docs/waves.md`, `README.md` |
| Steer as you go, plan and demo, or let it run | `website/content.yaml` |

## Left out because unverified

- That you never have to type a command. Installing the agent, git and `gh`
  likely takes a terminal, and the capture itself lists "Capture new Tasks from
  Desktop conversations" as not completed.
- That Loopflow can start a project from nothing. Docs assume an existing git
  repository.
- Apple silicon vs Intel, notarization, download size.
- Whether Loopflow itself sends anything off the computer. The page says where
  it runs and that there is nothing to register, and stops there.
- That the repair on a failed check is on by default in the app (`lf ci watch`
  is a command someone runs). Page says "can start a repair".
- Linear's cost.

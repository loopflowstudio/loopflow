# growth wave memory

Jack named this Wave growth on 2026-09-26, over website and marketing. It
is connected to Linear and holds an empty first chapter: no KRs, no metric
targets, no Task started. `GOAL.md` says `paused: true`; on 2026-09-26
`lf status growth` reported the Wave as not paused and idle. Which one Jack
intends is unconfirmed.

## Positioning (decided 2026-09-26)

Jack Heart set these in one design session. Wording in quotes is his.
The October 7 homepage approval below supersedes these earlier homepage
constraints where its exact copy differs; the docs decisions remain separate.

- Loopflow is "a software instrument: it doesn't make the software for you;
  you make the software through it." Jack weighed "software orchestra" and
  found it too grand. Earlier nouns he used, "software factory" and
  "personal company builder," describe where it is heading, not the name.
- The noun is defined once at the top of a page and not extended. Jack:
  "all but 1-2 of the instrument metaphors gotta go. they have to come
  naturally; when they are forced its a huge facepalm."
- After the definition, every point is written "in the most casual,
  understandable terms possible."
- Copy is tuned on Kim as the reader, not on an experienced engineer. Her
  sentence, as Jack hears it: "wow it seems like you can do so much with
  software, i kinda wanna try it, but i dont really know how to do it
  right." It guides the copy and is never quoted back at the reader.
- Hard veto: no sentence about Loopflow's stance, virtue, or competitors.
  Jack vetoed "We'd rather be honest about that than promise you'll be ten
  times faster" as "internal speak, not talking about users' problems at
  all." He also rejected "The hard part is knowing how to do it right."
- The benefit is a sustainable way to build. "The point isnt that it comes
  from per se but about how it feels and works for them." No appeal to
  experienced builders or craftspeople as the authority.
- The rhythm of hands-on and hands-off work is a main emphasis. "The more
  automation you do the more engagement you kinda owe as debt." How much to
  hand off depends on the work: "You can zoom in and you can zoom out." The
  defaults for which steps need a person are where Jack's judgment shows.
- The loopflow definition is the one technical section, written for the
  technical crowd as graph-theory prose. Set notation was "too mathy."
- The site says why Loopflow works at company scale, as the reason
  bottoms-up adoption works.
- Copy never requires the reader to see themselves as stuck. A parallel
  positioning Session on 2026-09-26 recorded this reply, without naming
  the speaker, to an imagined arrival of "I'm working constantly, but I'm
  getting nowhere":
  "I dont think most people say that. Most peopel who are getting nowhere
  dont see it that way." Going in circles is a problem Kim has not met yet;
  wanting to do it right is the one she has now.
- The definition carries no musical word. Jack first wrote "you make the
  music through it" and corrected it to "software."
- "Free" always sits next to a plain statement that the coding tool it
  drives has its own cost.

## Docs (decided 2026-09-26)

- Jack: "The docs should be for Scott or Kim. Maybe has basic CS 101
  knowledge, and is a smart knowledgeable millenial or gen-z information
  worker, but not a trained software engineer. The goal of the docs should
  to help them use loopflow as well as me."
- One reader, no second track for professionals. The docs carry that
  reader from a first task to Jack's level of use, so they explain why and
  not only how.
- The Mac app is the primary experience. Jack: "I am now confident in the
  Loopflow desktop exerpeince as the primary experience. However, note
  that it does use embedded terminals ias its way of interacting with llms
  directly." Docs lead with the app and teach the terminal inside it.
- The content stays. Jack: "the pages should also be literal and
  technical. The voice is friendly, but the content is still the same
  basically." He then called that "a slight overstatement but
  directionally correct vs turning docs into more marketing." Content may
  change where it serves the reader. The docs never sell.

- "How Jack works" is a blog post, not a docs page. Jack: "i think how
  jack works is probably more of like a blog post." Docs teach the
  product; his own practice is told in his voice, elsewhere.

## CLI account naming decision (2026-09-30)

Jack Heart: “i thought we decided on lf account instead of lf identity”.
LOO-338 uses `lf account` for provider logins, status, connection, routing and
its overview. `identity`/`id` are superseded draft names, with no account short
alias. `lf home id` retains machine identity. Jack also requires a complete
Clap-derived command and option catalog, including internal surfaces, with
caller evidence and keep/rename/merge/delete verdicts before implementation.
Jack's later September 30 steers reject prefix-only reorganization and waive
review/demo waiting. Guides use the shortest uniquely resolving commands;
canonical ownership stays in reference/help. Keep `wt`, `top` and `ps`; `mon`
is a derived prefix. The newest selectors are `--task` / `--wt` for location
and `--wave` for context and identity, with no `--as` alias.

LOO-338 now integrates the landed Exec/AgentSession/FlowSession and Account
models. Monitor separates live processes, recorded outcomes and missing
observations; active conversation streaming, raw/final Session evidence, usage
and historical Work queries retain distinct readers. Account and route each
have one reader. Saved per-provider choices survive retry/background children;
remote launch still needs destination access. Catalog and exact counts live in
the Task's scratch until delivery; the command reference is generated from Clap.


## CLI guide decisions (2026-09-29)

Jack asked the CLI guide to lead with complete workflows and the outcomes
Loopflow owns, keeping individual controls available for intervention and
recovery. Jack then rejected narrative and sales-like prose: ordered command
examples with brief explanations carry this guide. That correction changes its
presentation; it preserves the audience and hands-on/hands-off direction above.
Jack cut the dedicated Wave showcase section for now, while keeping its detailed
reference and capabilities in scope. Existing page addresses remain valid.

The [CLI guide](../../docs/lf.md) and [reference](../../docs/lf-reference.md)
describe the implemented owner tree. A disposable public-CLI walkthrough on
2026-09-30 produced a real Codex-authored README in a fresh Git project without
Linear or Task setup, then read Monitor, cached Account and each owner on that
state. Synthetic tests separately cover account restrictions and continuation.
This establishes the first local result; live OAuth, remote-Home execution,
rendered Desktop and a complete autonomous release lifecycle are different
proofs and are not claimed by that walkthrough.

Jack's subsequent authoring decisions use `cmd:` for command steps and one
Target model for commands, skills, flows, and XOR composition. Local source
implements these decisions and direct review/repeat fields on steps. Saved
execution plans still capture instruction bodies before running. These internal
changes do not establish any additional newcomer journey; the public guide keeps planning setup separate from the first local result.

## Docs rewrite plan (2026-09-26)

- On every page: open with when you need it, say each term plainly once
  and link the glossary, lead with the Mac app and show the command beside
  it, and give the reason behind each default.
- Page addresses do not change. Architecture pages stay as the developer's
  path.
- The reader has variables, loops, files, and what a program is. The
  reader lacks Git habits, pull requests, CI, code review, and the reasons
  engineers work the way they do. A page explains why wherever an engineer
  would already know.
- A walkthrough claims only a path that works today, start to finish.
- What a page drops is dropped because the reader does not need it, never
  to seem simpler, and never by leaving out what Jack would do.
- "Instrument" appears at most twice per page.
- The README and the docs index open with identical words.
- Tables and flags on reference pages are left alone; the voice pass
  covers openings and explanations.
- Two pages are new: the glossary, and "The terminal inside Loopflow,"
  which covers what it is, why it is there, how to type to the AI, how to
  interrupt it, and how to leave.
- [PR #1297](https://github.com/loopflowstudio/loopflow/pull/1297) carries
  the first pass: the glossary, the overview, the top of Conducting, the
  install page, and the removal of "no central server" from the Agent API
  page. It was open on 2026-09-26. Get started gained a list of what to
  have before installing, and is otherwise not rebuilt.
- Forbidden: a beginner track beside an expert one, docs that read like a
  second homepage, a walkthrough that ends at a step the reader cannot
  take, and a Mac app walkthrough written from code without opening the
  app.

## Tasks filed 2026-09-26

Jack asked for six or fewer.

| Task | Scope |
| --- | --- |
| LOO-306 | Get started in the Mac app, and the terminal inside Loopflow |
| LOO-309 | The rest of the docs in the new voice |
| LOO-311 | Jack's way of working, as a blog post |
| LOO-312 | From download to a running Loopflow |
| LOO-316 | A first task needs nothing but your computer |
| LOO-317 | The Mac app shows a flow as only the steps that need you |

LOO-307, 308, 310, 313, 314, 315, and 318 were merged into these and are
titled "Merged into". `lf pm task` has no cancel, and closing them as done
would record unstarted work as finished, so they wait for Jack to cancel.

## Audience and business

- Starters come first: people who "don't really know how to code but
  totally could."
- A few named people are reached one at a time. What Jack wants from each
  is unrecorded.
- Professionals and big companies are welcome and never chased. They
  should replace every skill and flow over time; the defaults aim to be an
  "80% reasonable starting place."
- "OpenClaw : individual :: loopflow : company," and "still free and
  bottoms-up."
- "Not selling the personal company builder." Revenue comes later, only
  from connecting several people's individual setups into one shared
  company. The site does not state this plan.

## Homepage approved 2026-10-07

Jack Heart approved prototype 05 ("this looks great"), recorded in LOO-425.
Its copy is now preserved in `website/content.yaml`: "Loopflow is a software
instrument" above "A command line for great software engineering." The subline
is "A free, open-source podium for conducting your software orchestra, with
native macOS and terminal interfaces." Preserve Jack's wording; add no musical
language. This approval supersedes the September homepage order and the earlier
rebuild in [PR #1410](https://github.com/loopflowstudio/loopflow/pull/1410).

The page leads with commands, then ownership, Flows in the terminal, four rows
(Wave, Project, Task, Workflow), the Mac app, why, and install. The three-frame
animation, Discord room, and separate autonomy comparison are removed. No Linear
name, review UI, testimonials, or invented captures belong on the page. The Mac
download remains available and body mentions of `lf` use monospace.

The approved prototype reserved `website/static/cmux-flow.png`, captioned
"A Flow running in cmux." The later October 7 change (`ff4483fd7`) removed that
reference and its reserved space: the homepage now ships without a terminal
capture. That commit records that interactive launch and presentable Flow output
in cmux remain unresolved. No cmux sidebar behavior is claimed.
The Mac image remains the unedited October 1 capture with its provenance sidecar.
Browser layout and CLI help were checked; this does not establish a fresh-machine
install journey or an observed cmux Flow run. LOO-425's directive ends at PR
publication, despite the broader outcome mentioning production deployment.

Implementation review on October 7 found that Pico's inherited heading margins
and inline-code boxes changed the approved layout. Homepage resets are scoped
under `.home-page` because docs and download pages share the stylesheet;
`docs/architecture.html` embeds it and needs regeneration when it changes.
CI then exposed fallback-font differences: the hero buttons were 43px tall on
Linux despite passing on macOS. An explicit 44px minimum and fallback-font
coverage preserve the touch target across platforms; run the website suite
after the final layout edit.

## Earlier copy decisions (2026-09-26)

Word for word: "AI can build a lot of software fast. It can also spend all
day going in circles, and it's hard to tell which is happening. Loopflow
keeps track, so you can see what got done and what still needs you." It is
the earlier tone reference; its homepage problem/solve placement is superseded
by the October 7 approval.

Jack chose "One task at a time" and "Make the plan," and left "Let it
build" as "fine for now." The other September homepage lines were drafts he had seen
and not approved line by line.

- The September homepage followed the take Jack called "probably closest": lead with
  the rhythm of one piece of work, shown as steps. Two other takes, one
  leading with the problem and one short invitation, were set aside.
- Jack wanted the circles paragraph to be "more of a features list or a
  problems or a solutions slot," which is why it is a pair and not a
  standalone paragraph.
- The same parallel Session recorded this invitation as assessed
  "reasonable," speaker unnamed; it is a draft direction with no place on
  the page yet: "Bring something you want to make. Loopflow helps
  you work through it, with a plan, checks along the way, and room to
  change your mind." The headline drafted above it, "You don't have to
  figure out the whole process before you start," is not approved.
- The two-paragraph loopflow definition is "not perfect but ok for now."
  Jack cut a third paragraph about the people-only view: that idea ships as
  a view in the Mac app, not as words on the site.
- A noun study before Jack's choice ranked "software workshop" first, then
  "software agent with a routine" and "software agent that shows its
  work." Instrument supersedes them.

## What stands between a newcomer and a first task (found 2026-09-26)

Verified in code unless marked. The Tasks filed above came after this
list; which item each one carries is read from its title, not confirmed.

1. The installer puts `lf` in `~/.local/bin`, which is probably not on a
   fresh Mac's path. Inferred, untested on a clean machine.
2. The Mac app's empty states are sentences with no button. Creating a Wave
   is only possible in a second window.
3. Nothing installs or signs in to a coding agent.
4. Nothing creates a project from nothing. Every path assumes an existing
   Git repository on a branch named main.
5. Every Task is a Linear issue, and Linear sign-in needs developer
   credentials a newcomer does not have.
6. A Task cannot finish without GitHub.
7. With no coding agent installed, the error names only Codex. When one
   is installed, Loopflow now uses it without being told, preferring
   Codex, then Claude Code, then OpenCode.
8. The Mac app has no model of a flow's steps. A people-only flow view,
   where autonomous steps collapse into edges and a backward edge survives
   only when no person sits on its path, needs steps on the wire first.
9. `lf doctor` lists Rust, uv, and Doppler as required. That list also
   generates the Brewfile and describes a maintainer's machine.
10. Nothing gives Kim something to show a friend after a week.

## Open

- What a first task in the Mac app looks like, click by click. Nobody has
  opened the running app to write it down, and the walkthrough cannot be
  written from code.
- Whether a first task needs a planning account. Every Task is a Linear
  issue, while a flow run directly does not need one.
- Whether PR #1297 lands now as the front-door change, or waits.
- Reading the homepage aloud to Kim or Scott has not happened. The test:
  read it from a phone, no screen shared, ask "What is it, and what would
  you do with it?", write down their words exactly, and ask again the next
  day what they remember.
- Starting with only Claude Code installed was proven with a stand-in
  `claude` in an isolated home, not on a real Mac.
- GitHub reporting the license as MIT can only be checked after the
  `LICENSE` file reaches main.
- What "do it right" means to Kim is unexplored.
- Whether the definition of "instrument" should rest on the rhythm idea:
  an agent works for you, an instrument stays in your hands.
- Whether "conducted by you" or "craftsperson" returns anywhere.
- Who the named few are beyond Dave Braginsky and Dustin Moskovitz, and
  what each is shown.
- "Instrument" also names a measurement in the product ("metric
  instruments").

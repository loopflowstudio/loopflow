# lf: the CLI your agents use to get work done

Status: draft, 2026-10-24. Direction from Jack; copy below is Claude's draft.

## What to build

The homepage and the Agent API page pitch `lf` to the developer who tells their
agent "use lf". The agent learns to break work into phases with contracts and to
launch subagents for them. Because each phase's state lives in its artifacts and
not in the agent's context, any agent can pick the work up again after a failure.

## Jack's words

> I want to explore a take at marketing lf as "the CLI your agents use to get
> work done".

> Stop thinking of it so much as the tool that developers inside cmux use
> (though it can still be that), but more as the thing that you tell your
> agents to use to launch subagents

> You're encouraging your agents to break the work into discrete phases with
> clear contracts such that if there are any failures the work is immediately
> resumable by another agent. Then the LF resume, the agent, and the auth
> system are making it, again, easy to recover across many parallel agents

> [On the last pass] we did things I wanted to go towards, like Loopflow for
> Teams. I think overall I liked it and I'm happy with having made that step.
> I do think in the docs we maybe got ahead of ourselves. I think the docs
> portion should probably be more transparent, less vision-oriented, and more
> about what exists and how to use it. Just imagine you are a coding agent and
> your user has told you, "Hey check this out. Should we use this?" Then the
> coding agent should be like, "Yeah it seems useful. It has a clear use case.
> It seems like it will be easy to try and will work for this small thing.
> It's worth doing."

## What we are selling

Jack, 2026-10-24: "I think we are fundamentally selling the idea that your
agents build on the work of each other, but we're doing it in this particular
way." The particular way: phases that leave durable artifacts in git (design
doc, branch, commits, PR), and a persistent conversation per Task that is free
while background runs do the work. The landscape to position against is shared
context, memory management and agent-to-agent communication (research in
progress: `scratch/reports/Agents building on shared work.md`).

### Combining the best of four categories

Jack, 2026-10-24, on the landscape summary: "this is actually not that far
from a good way to explain all the different things loopflow does", then "I
would rather think about how to use these categories to basically say
'loopflow combines the best of these other tools'".

| From | The best part | In `lf` |
|---|---|---|
| Memory layers | What agents learn outlasts the session | Wave `MEMORY.md`, design docs: readable files in git |
| Agent messaging | You can reach an agent while it works | `lf comment` reaches the Task's conversation before its next step |
| Task trackers | One task, one context, one PR | One owner, worktree, PR per Task; plan in Linear or a Git ref |
| Orchestrators | Many agents at once, each in its own worktree | `lf launch` on any provider; opens in tmux/cmux/Herdr |

Plus what only `lf` does (Jack, 2026-10-24: "along with Skill Graphs ... maybe
1-2 other things we claim ownership over"). The draft claims:
Skill Graphs; a persistent conversation per Task, opened in your multiplexer;
your own logins across providers with routing. `--machine` and rented pods are a
possible fourth.

Jack, 2026-10-24, on "In lf: each Task has one owner, one worktree and one pull
request…": "something like this just doesnt sell me at all". Copy rule: lead with
what the person gets (ten PRs you can review, a change of mind that doesn't
restart the work). Mechanism is supporting proof at most, never the headline.

Jack, 2026-10-24, the framing: "It's more like lf connects:" with his first item,
kept verbatim in the README draft:

> * Task tracking: lf automatically makes tasks for you as you define work so
>   that the work can proceed in the background and have a dedicated session you
>   can easily find

Memory, Messaging and Orchestration items are Claude's drafts in that voice
(`scratch/README-draft.md`). Today, "automatically" means the agent runs
`lf task create`. `lf launch`, or Task creation inside the loopflow skill,
makes it literal.

Categories as of 2026-10-24 (Jack): "Work, Context, Orchestration, and Sessions"
(were Task tracking, Memory, Orchestration),
Sessions, plus Skill Graphs as what joins them. Messaging was "a little soft
as a category … its mostly memory", so comments fold into Memory. Visibility was
added, then dropped: "i dont think i actually use visibility that well". The
observation commands stay as reference. Orchestration split into Orchestration
(agent, login, machine, worktree) and Sessions (outlive the terminal, `lf resume`,
`lf open`). Jack accepted moving "a dedicated session you can easily find" out of
his Task tracking line into Sessions. Sessions also gets the account switcher
(Jack): logins move from Orchestration to Sessions. Orchestration also takes
`lf land --wait-and-fix`, the Wave operator passes and Graphs moving runs step
to step (Jack). Memory felt soft and became "Context management" (Jack): context is assembled
for every launch and refreshed after compaction. "Memory" is the crowded
vector-store category. Each category gets one line in
the approved shape: "Waves share a collective memory; within a Task, lf's
built-in skills hand off through shared contracts in scratch/."

The page names categories, not competitors. Avoid "remember / never forgets"
(memory category), "software factory" and "army / 10x" (research report).

## Two surfaces, two jobs

- **Homepage:** keep the direction from the last pass (#1637), including
  Loopflow for Teams. The agents-use-lf pitch is added there. The homepage may
  show intended product.
- **Docs, `/llms.txt`, `/llms-full.txt`:** only what exists and how to use it,
  in the present tense. The reader is a coding agent whose user asked "should we
  use this?" These pages are the product page for that reader.

### The evaluating agent's questions

The docs answer these in order, starting on the first screen of `/llms.txt` and
`docs/index.md`:

1. What does it do, in one sentence a user would say?
2. What problem does it solve that my built-in subagents don't?
3. Does it work with my setup? (harness, OS, git, gh; planning optional)
4. What is the smallest trial on this repository, and what will I see?
5. What does it touch? (`~/.lf`, `.lf/`, `scratch/`, worktrees beside the
   repo, the user's own provider login. No hosted server.)
6. How do I undo it?
7. What is not built yet? An explicit short list, instead of features
   described as if they exist.

Internals (Process, LfSession, cutover status, authority fencing) move out of the
entry pages into `architecture*.md`. They stay true, but they're no longer what
a newcomer reads first.

## The argument

Parallel agents fail all the time. Their context runs out, a login hits its
limit, the laptop sleeps, the agent goes off the rails, or the machine is
preempted. With built-in subagents, the work dies with the conversation that
held it.

`lf` takes two layers to make failure cheap.

1. **Phases with contracts (the process).** Skills and Graphs split work into
   steps such as design → implement → gate → ship. Each step produces artifacts
   on disk: a design doc in `scratch/`, a branch, commits, a PR. The next step
   reads those artifacts instead of the previous agent's memory. A stopped Graph
   is history, so nothing has to resume a dead process. A fresh agent reads the
   contract and continues. (This is already the architecture. The pitch puts it
   up front.)
2. **Recovery across many agents (the machinery).**
   - `lf resume --all` brings back every conversation a crash interrupted.
   - `-a codex` runs the same step on a different harness when one is stuck.
   - `lf account` routing moves to the next login when one runs out.
   - `--machine` runs the step elsewhere.
   - `lf task run` starts the Task again from its durable direction, worktree
     and PR.

Supporting idea, not a headline (Jack, 2026-10-24): "work that outlives your
agents". Use it in body copy or a section lead; the headline stays open.

### No comparison table

Jack, 2026-10-24, on the raw / built-in subagents / Loopflow table: "i think that
whole table basically is wrong, kill it". `/cli` makes no feature-by-feature
comparison with built-in subagents. The one contrast it keeps is with using an
agent directly: that conversation is busy doing the work.

## Audience shift

Before: a developer at a terminal (or in cmux) typing `lf` commands, conducting
the work.
After: a developer who adds one line to their agent setup; the agent types
the `lf` commands. People still type them; the page just stops assuming they do.

Front door:

```bash
npx skills add loopflowstudio/loopflow --skill loopflow -g -y
```

or a line in `CLAUDE.md`: "Use `lf` to launch subagents and break work into
phases."

## The demo

`uv run python dev.py serve` in `website/`, then open `localhost:5001`. The hero
reads "The CLI your agents use to get work done." The first picture is a
Claude Code transcript, not a person's terminal. In it:

1. The agent splits a change into two phase-contracted subagents
   (`lf -b --wt … skill implement`, one on `-a codex`).
2. One subagent dies when its login runs out. The transcript shows `lf` moving
   to the next account, and a fresh agent continuing from the design doc and
   branch the dead one left.
3. The parent waits without polling (`lf task wait … --json`) and hands back
   two PRs.

The recovery beat is the part to prove. Without it, this is just another
parallel-agents page.

## Homepage decision

Jack, 2026-10-24, after three takes (`scratch/website-takes/`): "A is the right
overall frame". The headline is "The CLI your agents use to get work done", and
the hero picture is a transcript of an agent splitting the work, a subagent
failing, and another one finishing. Take B's "Every phase has a contract"
section is "a good paragraph somewhere". It now sits in take A after the
built-in vs `lf` comparison. B's phase diagram and recovery cards are not used.

## /cli as its own page (exploring)

Jack, 2026-10-24: "i want to try going back to the lf CLI as a separate page",
"maybe bring back just like showing what the lf command does".
The draft is `scratch/website-takes/cli.html` and `home-split.html`. The
homepage keeps the teams story, and its `lf` section becomes a band linking to
`/cli`. `/cli` takes take A's frame, then the sections below, then a "What `lf`
does" command block (the pre-#1637 `/cli` list, updated to current commands).

Jack, 2026-10-24: "this page should lean into the fact that there is a separate
page for Loopflow desktop, we dont need to sell e.g. teams". `/cli` sells only
the CLI. Teams and the shared plan belong to the homepage (Loopflow for Mac).
`/cli` ends with one pointer there.

Jack, 2026-10-24, on "The agent you talk to isn't the one doing the work": "I
also dont think this is really a selling point. Maybe 1-2 sentences can go
somewhere else." The section is gone. Its point survives as the lede of
"Forgot where you left something? Ask.": each launched Task gets its own
persistent conversation, picked up with `lf resume --task` or found by asking.

### Inverting the agent hierarchy

> I think there's something about inverting the agent hierachy

> Claude in particlar really likes to try to encode its subagent history in a
> way taht is not really trasnaprent to you

Built-in subagents report to their parent. Their transcripts are folded into
its session, and you get the parent's summary. With `lf`, the agent that splits
the work is one Session beside the ones it launched. You can open, read, steer
or take over each of them (`lf session list`, `lf resume --task`, `lf comment`).
On `/cli` this section comes right after the transcript.

Correction from Jack, 2026-10-24:

> it doesnt give each a subagent, its give yous you a dedicated session that is
> also not busy doing the work, so you can interrupt it

The inversion: the conversation you talk to never does the work. It hands the
work to background runs and stays free, so you can ask, redirect or interrupt
(`lf task interrupt`, `lf comment`) mid-run. Copy says "background runs", not
"subagents", for what `lf` launches. "Subagent" names only the built-in
comparison.

## `lf launch`: create and run in one verb (product change)

Jack, 2026-10-24, from the transcript draft: "this is making me [think] we need
launch = create and run", "so its just launch".

Today the transcript needs two commands per piece of work (`lf task create
--title …`, then `lf -b task run ETU-14`). Nothing combines them: `lf task run`
requires an existing Task, and `lf pursue "…"` runs a Graph with no Task identity.

```bash
lf launch "rent CPU pods for templates with no GPU"     # create the Task, then run it
lf launch "backfill pod kind" -a codex --wave runs       # same flags as task create/run
pbpaste | lf launch                                      # report from stdin; first line is the title
lf launch ETU-14                                         # an existing Task: run it
```

`lf launch` prints the Task identifier as the first token of its output
(Jack, 2026-10-24: "needs to return or print the task-id"), then the workflow
and the step it is starting. `--json` returns the same as an object. The
calling agent needs the ID to `lf task wait`, `lf comment`, `lf task interrupt`
or `lf resume --task` it:

```
$ lf launch "rent CPU pods for templates with no GPU"
ETU-14  launched · feature · design
```

Every launched Task also has its own conversation (Jack, 2026-10-24: "show
you like easily connecting to that task's session, and it saying 'yep,
currently running stage X'"). `lf resume --task ETU-14` opens it, and it
answers with the run's current step, because the run, not this
conversation, is doing the work.

Opening it for you (Jack, 2026-10-24: "the next thing is opening the session
for you in tmux"): `lf open --task ETU-14` already exists. Run from tmux, it opens
the Task's conversation in a new window right after the caller's, and from cmux
or Herdr in a new workspace there. Otherwise it opens in Desktop or a terminal.
The transcript has the agent call it after `launch`. Open: should `lf launch
--open` (or opening by default when the caller is in a host) fold it in?
Note that Jack said on 2026-10-07 that tmux "made the terminal experience worse".
That was about Loopflow running its own multiplexer. Opening into the host you
already use is consistent with it.

Leaning, not decided: `lf launch` takes over `lf task run` (one verb, Task ID or
new title), and `task run` is deleted rather than aliased. `lf task create`
stays as the plan-only verb (file it, don't start it).

Open:
- **Blocking or detached?** The pitch is "your conversation stays free". A
  launch that blocks until the Graph ends makes the calling agent busy unless
  its harness backgrounds the shell. Detach by default and use `lf task wait`
  for blocking? Today `lf -b task run` returns when the Graph ends.
- **Title vs. Task ID:** a bare argument matching a Task identifier runs that
  Task. Anything else is a new title. Is that ambiguity acceptable, or does an
  existing Task need `--task`?
- **Name collision:** check that `documented_commands` still resolves the
  `launch-plan` skill and `lf launch` uniquely.

Placement: this is a product change bigger than marketing. It probably becomes
its own Task, landed before the `/cli` page claims it.

## Context-aware (selling point)

Jack, 2026-10-24: "ok so new selling point is its context-aware".
The behaviors below exist today. On `/cli` they form a section after the
hierarchy section:

- `lf open`: host from caller environment (tmux → next window, cmux/Herdr → new
  workspace, else Desktop, else terminal).
- Inside a Task's worktree, Task commands need no ID, and `lf resume` reopens
  that worktree's conversation.
- Commands from an agent's shell record that conversation as their parent.
- `lf comment` from an agent's conversation counts as progress, not a steer.

Jack, 2026-10-24: "automatically getting persistent sessions for tasks made by
your coding agent is good". Every Task an agent launches gets a persistent
conversation with no setup from you. `/cli` says this in the hierarchy section
and its caption.

Jack, 2026-10-24: "we may also need more like showing the list of tasks to task
ids". `/cli` shows `lf task list` with ID, title, current step and where its
conversation is (tmux window, PR, or waiting on you). Today `lf task list`
prints only `ID · owner · title`. The step, the state and where its
conversation is open are a product gap to close alongside `lf launch`.

The scenario, in Jack's words: "i was working on X somewhere but i forgot
where. could you open that task for me". The agent runs `lf task list`, matches
the description, names the ID and runs `lf open --task`. This is shown on `/cli`.

`lf launch` with no argument inside a Task worktree depends on `lf launch`
existing (see above).

## Line candidates

- Jack, 2026-10-24: "The CLI by agents for agents by humans for humans."
- Supporting idea, not a headline: "work that outlives your agents".

## Page structure (sketch)

1. **Hero:** "The CLI your agents use to get work done." Sub: "Tell your agent to
   use `lf`. It breaks the work into phases, launches a subagent for each, and
   when one fails, another picks up where it stopped." Install line, then the
   skill line.
2. **Transcript picture:** split → failure → recovery → PRs.
3. **Phases with contracts:** the shipped workflows card (`feature`, `code`,
   `research`), reframed as contracts between agents, with stops where a person
   belongs.
4. **Recovery machinery:** resume, harness, accounts, machines. One terminal
   block. The current `lf` CLI hero's flag stack fits here.
5. **Working together:** Tasks, Waves, shared plan and comments. They keep a
   swarm and a team coherent. This is today's "Arrange your coding sessions…"
   section, compressed.
6. **Works with:** unchanged. cmux and herdr stay listed: `lf` still runs inside them.

The Mac app mocks move below step 4 or out of the hero. They show where you
watch the work, not the pitch itself.

## Constraints

- Pictures are hand-drawn HTML and may show the intended product (website
  README). Recovery claims still need a real path today: `lf resume --all`,
  `lf account` routing, `-a`, `lf task run` from durable direction. Verify each
  before it goes on the page.
- `docs/agent-api.md` stays the reference. Its opening shifts toward
  examples first, the phase/contract argument, and then the existing contract
  detail.
- `skills/loopflow/SKILL.md` is the actual front door. It should teach phase
  decomposition and recovery, not only "use lf". It is a builtin-style skill,
  so it has to be self-contained.

## Current system

- `website/home.html`: hero "how you design, implement, demo, release and
  study software in harmony", addressed to a person conducting.
- `website/content.yaml`: `homepage.title`, studio copy ("a software
  engineering CLI for your agents" already leans this way).
- `docs/agent-api.md`: opens with "`lf` is the API agents call…", then mostly
  internal contract.
- `skills/loopflow/SKILL.md`: generic operating rules, no decomposition or
  recovery teaching.
- `README.md` lead paragraph: "Build software with other people and their coding
  agents."

## Delete — do not maintain

- The rotating-verb hero and "Conduct an orchestra that conducts itself".
  Replaced, not kept as a second headline.
- The duplicated `lf` command walkthrough. Today's `clihero` section is folded
  into "Recovery machinery" rather than kept alongside it.

## Forbidden outcomes

- Two pitches on one page (people-conducting hero plus agents section below it).
- Vision in the docs: a docs sentence describing behavior `lf` doesn't have
  today, outside the "Not built yet" list.
- Claiming automatic recovery `lf` doesn't do. The architecture says a stopped
  Graph is never resumed; the honest claim is "another agent continues from the
  contract", started by `lf task run` or the parent agent.

## Internal slices

1. **This slice: the docs front door.** Rewrite the `generate_llms_txt` summary
   in `website/main.py`. Rewrite the top of `docs/index.md` and Getting Started
   → Try It so they answer the seven questions. Open `docs/agent-api.md` with
   the delegate → wait → read-back example and the phase/contract argument, and
   move the authority detail below it. Add a "Not built yet" list. Verify every
   command shown against `lf help`.
2. `skills/loopflow/SKILL.md` teaches decomposition and recovery.
3. Homepage: the agents-use-lf hero and transcript, alongside the teams
   direction. README lead paragraph to match.

## Done when

- `cd website && uv run python dev.py test` passes.
- **Evaluation check:** a fresh headless agent, in a scratch repository, is given
  only `https://loopflow.studio/llms.txt` (served locally) and the prompt "My
  user asked: should we use this? Answer, and name the smallest thing to try."
  It recommends a concrete trial command that exists, states the requirements
  correctly, and claims no feature from the "Not built yet" list. Run it before
  and after the change; the before run is the baseline.

## Open questions

- Is the recovery-after-a-login-runs-out story true end to end today for a
  headless subagent, or does it need `lf task run` from the parent? This decides
  whether the transcript shows an automatic or an agent-driven restart.
- Does the parent agent learn a subagent failed without polling? `lf task wait`
  covers Tasks. What about a bare `lf -b skill …`?
- Tension: Jack's earlier framing (2026-10-07) has the launcher as the technical
  center and the work model as the product center. This pitch leads with the
  launcher plus process. Here, Tasks/Waves become the coordination layer below
  the fold. Jack to decide.

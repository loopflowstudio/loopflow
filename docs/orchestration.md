# Orchestration

You've launched a Skill and a Graph. This page makes them yours: which agent
runs each step, which machine runs the Graph, how a Task moves from idea to
landed PR, and how a Wave keeps going.

Loopflow ships reasonable Graphs for software engineering, and they are
templates. Copy one with `lf graph customize pursue`, change it, or write down
the way you work and share it.

## Skills

```bash
lf skill customize pr-message    # writes .lf/skills/pr-message.md, an extension of the builtin, and prints the path
lf skill customize gate --full   # copies the builtin's whole text instead
lf audit focus on the token refresh path   # your .lf/skills/audit.md, with a message
lf -a codex audit                # the same Skill on Codex
```

Customize writes once, then keeps printing the existing file. A repository
Skill without `extends` replaces the builtin of the same name, for everyone who
has your commit; `--full` starts you from the builtin's text.

The file customize writes adds to the builtin and keeps following it as it
changes. Write only your section:

```markdown
---
extends: builtin
---
## In this repository

Run `make e2e` before publishing.
```

That file is the builtin with your section appended. `lf list skill` says
which of your Skills extend a builtin and which replace one:

```
    lf skill gate (.lf/skills/gate.md, extends builtin)
    lf skill qa (.lf/skills/qa.md, replaces builtin)
```

One Skill, one job: `design` writes the plan, `implement` builds it, `gate`
judges whether it ships. Chain them in a Graph rather than writing one Skill
that does everything. Pass work between steps through what already holds it:
commits, and notes in `scratch/`.

Each Skill step's edits become one commit, pushed unless you're on the default
branch:

```
lf implement: Count retries per Task

The board read attempts from the wrong row.
```

The `commit-message` Skill writes it on its own agent, so a cheap model can
take that job alone: `lf skill customize commit-message`, then set `agent:
claude:haiku` in its frontmatter. To keep [`agents`](config.md#keep-going-when-a-provider-runs-out)
behind it for when Haiku is refused, set `default_agent:` instead, or
`lf set --global skill.commit-message.default_agent claude:haiku`. PR titles
and bodies take the same settings under `pr-message`.
Without an answer the title is `lf implement: checkpoint`. `lf commit --step
implement` runs the same thing by hand; plain `lf commit` has no prefix.
The commit is no step of the Graph: its recorded steps stay the ones it
declares.

## Pick the agent for each step

```markdown
---
agent: codex
---
Audit auth changes on this branch. Fix missing validation and untested paths.
```

```yaml
# .lf/graphs/ship-api.yaml
- step: {name: implement, agent: claude:opus}
- audit                         # its own frontmatter: Codex
- gate
```

A Skill's frontmatter `agent:` names its harness and model; a Graph step can
name another for that occurrence. Explicit Graph `-a` applies to all its steps,
including nested Graphs and routers. It is not exported to unrelated child work.
Personal preferences can target the skill or an authored step ID;
see [setting precedence](config.md). Use `default_agent:` in either definition
for a fallback: every applicable `agent:` choice wins before any fallback,
then source, scope and target decide within each class. Headless work on a fallback
moves through [`agents`](config.md#keep-going-when-a-provider-runs-out) when a
usage limit or unavailable model refuses it; work with `agent:` stays on its
provider and keeps its model when one is named. Builtin skills recommend
no harness; the application's fallback applies when nothing else selects one.

```markdown
---
context:
  docs: [scripts/testing.md]
---
Run the affected suites once for this tree.
```

A repository Skill's `context.docs` adds files to its context, resolved like `--docs`.

## Run a Graph anywhere

```bash
lf -b graph ship-api                     # headless, step to step; returns when it ends
lf -i graph ship-api                     # every step opens in the agent; exit it to advance
lf --machine mini -b graph ship-api      # the whole Graph on another machine you added
lf graph ship-api --context --json       # what the first step would receive; launches nothing
```

Without `-i` or `-b`, steps run headless except `interactive:` ones, which open
here ([below](#where-a-person-takes-part)). A failed step stops the Graph. A
stopped Graph is history: inspect it with `lf graph show <RUN_ID>`, then
start fresh work. Nothing resumes it.

Mechanical steps ride along as commands:

```yaml
# .lf/graphs/deliver.yaml (the builtin)
- reflect                     # turn what you said during the Task into skill edits
- gate                        # publish so CI starts, then review while it runs
- cmd: land --wait-and-fix    # merge, repairing CI until it lands
```

[What gate does and reports](ops.md#review-while-ci-runs).

## Loops and branches

A `loop:` block runs again while its last step says so. `build`, the second
half of `pursue`, is two of them:

```yaml
# plan (builtin)
- kickoff              # plan the change, risks answered
- concept-review       # question the plan's premise before any code
- write-docs           # edit the actual user docs before implementation

# build (builtin)
- loop:
    - loop:
        - implement        # one chunk of the plan
        - cmd: sync
        - realign          # decides: next chunk, or on
    - compress
    - qa                   # use the change through its real path
    - concept-review       # decides: the finished change, whole
- pr-publish
```

`write-docs` edits the actual README, guides, or reference files before code:
examples, expected results, and what someone needs to know to use the change.
In `pursue`, this happens right after kickoff plans the change. An already attached Loopflow
Desktop shows the relevant files in Files, with rendered Markdown/HTML and
existing drafts preserved. In other hosts, or without an attached Desktop,
the conversation reports the actual documentation paths and continues. Automatic
presentation never launches an editor, host, browser or conversation.

Edit the docs or discuss changes while work continues. There is no extra review
stop or scratch copy. Saved edits guide implementation; for a change that needs
attention now, tell the Task conversation. `lf open --file <path>` presents the
original in an attached Desktop Files view. Internal changes with no
documentation impact leave the docs alone.

`qa` keeps what it learns about reaching each part of your product in
`.lf/qa/`, one file per surface. The first line says when to read it, so a run
lists them and opens only the ones its change touches:

```
$ head -n 1 .lf/qa/*.md
==> .lf/qa/desktop.md <==
Read when the change reaches Loopflow Desktop: its windows, panes or bundled `lf`.

==> .lf/qa/hooks.md <==
Read when the change touches turn or tool hooks, or what they record.
```

When a step no note describes had to be worked out, or a note stopped working,
`qa` writes the working version into that surface's file and commits it with
the change. Edit them like any other file. A repository without `.lf/qa/` gets
its first note when `qa` has something to keep.

The last step of a block does its own work, then returns advance (go on),
iterate (run the block again, with direction) or blocked (with the question).
Loopflow tells it what each decision means, so any Skill can decide; a command
can't. There is no pass limit; iterate repeats as long as there's useful work.

`xor:` picks exactly one path. The router Skill reads the evidence and returns
`{"path":"NAME"}`; a path with nothing to run exits cleanly:

```yaml
- qa
- xor:
    router: triage
    paths:
      repair:
        graph: code
        description: "Reproduced defects within the change need repair"
      silence:
        description: "No actionable defect in the supplied change"
```

## Move one number

```bash
lf graph optimize : "cut p50 export time on the 60k-row dataset"
```

`optimize` measures, plans one change, makes it, and keeps or reverts it, until
no clear way forward remains. It then compresses, refreshes and measures the
whole result. It runs without a person and delivers nothing, so compose it:

```yaml
# .lf/graphs/speed-up.yaml
- graph: optimize
- graph: deliver
```

## Where a person takes part

A step written `interactive: <skill>` is where a person takes part; every other
step runs headless. A Task's workflow is the Graph it took up and the step it
stands at.

```yaml
# feature (builtin)
- interactive: explore      # you choose what to build, for as many rounds as it takes
- graph: plan               # kickoff plans it, then docs
- loop:
    - loop:
        - graph: build      # code and a published PR
        - interactive: ship-review # you read the page; iterate goes back to build
    - graph: deliver        # gate, then land and stay until it merges
    - interactive: verify   # you check it on main; fix forward or finish
```

```bash
lf graph feature                    # explore and ship-review open here with you; the rest runs on its own
lf -b graph feature                 # every step headless; an agent stands in at explore and ship-review
lf task run LOO-12 feature          # runs up to explore and stops; the Task waits in its conversation
lf task run LOO-12                  # after you're done there: on to ship-review
lf task run LOO-12 --from explore   # back to choosing what to build
lf -b task run LOO-12               # the rest, straight through to a merged PR
lf ship LOO-12                      # auto on: the rest runs with an agent standing in for you
```

At `ship-review` you read one page, `scratch/ship-review.html`: what ships, in
your terms, and everything the run left outside the PR, each with a proposal to
fix it here, give it its own Task, or drop it. It is not a code review; concept
review and gate already answered whether the change is right. You decide what
happens to each finding. Ask for more and the change goes back to `build` with
your words in the plan; otherwise it goes on to `deliver`.

`-i` makes every step of a launch interactive and `-b` every step headless. At an
interactive step with a loop behind it, you, or the agent standing in, decide
advance or iterate; blocked there, the Task waits at that step. A step that
fails is tried again, up to three attempts, starting at that step; the steps
before it stay done. A skill step that failed before any agent started (its
harness is not installed, say) is not tried again, since the next attempt would
fail the same way: fix the cause and `lf task run`. When a headless OpenCode
or Pi agent asks a question, the run stops after one attempt and prints the
question. Answer with `lf steer LOO-12 "…"`, then `lf task run LOO-12`. A run
that stops leaves the Task at that step. `lf task status` names the step, and
the next `lf task run` starts there:

```bash
lf task status LOO-12               # Graph code: stopped at land --wait-and-fix
lf task run LOO-12                  # lands again; gate does not rerun
lf task run LOO-12 --from gate      # when gate's work must be redone
```

`--from` takes a step's id, its Skill, or the Graph it was composed from;
starting from an interactive step only puts the Task there. After the last step
the Task reaches `end`, which requests completion.

### Auto

Auto is one setting on the Task's Graph: on, off, or never set. Status says
what is happening; auto says who takes the interactive steps. While it is on,
the Task's runs stand in for you at each interactive step they reach and may
merge the PR. Turning it on means nobody is waited for, so it runs the Task:
`lf ship`, `lf task auto on` and Desktop's **Auto** toggle are one operation.
It prints what `lf task run` prints and leaves one Graph process:

```bash
lf ship LOO-12                      # LOO-12 · code from ship-review running as Graph run …
lf task status LOO-12               # LOO-12  moving · auto: on by Maya, Nov 18 09:04
lf task auto off LOO-12             # LOO-12: auto off
lf task auto on LOO-12              # the same command as `lf ship LOO-12`
```

A Task waiting at an interactive step is stood in for there, and one that
never ran takes up its Graph and runs. The run reads auto at each interactive
step, not at launch: turn it on while `build` runs and that run carries on,
standing in at `ship-review` when it gets there.
`lf task auto off`, or Desktop's **Auto** toggle, kills nothing. The run carries on to its next interactive step and
waits there, or reaches a merging step (`lf land`, `lf pr arm`, `pr-land`) and
waits before it: "Waiting at pr land: auto is off (Maya). `lf ship INF-1` lands it."
Auto-merge that Loopflow armed is disarmed. Off stands:
`lf -b task run` turns auto on unless someone turned it off, and only
`lf ship` turns it back on. Other Graphs in the Task's worktree never move it. There is no
approve command: give feedback in the Task's conversation, which knows its step
and how to go on.

Auto decides who takes the interactive steps; it is not what lets a PR
merge. Any run that reaches a merging step lands unless auto is off:
`lf task run` after `ship-review` goes through `deliver`, and the `auto` Graph, which has
no interactive step, lands without the setting ever being on. `lf task run` says
so on its `merges:` line. A failing run leaves auto alone: the Task stops at
the failed step, and the next `lf task run` stands in again. A PR merged by hand
is met by the run's own landing step, which sees the merge and goes on.

| Task state | When |
|---|---|
| `unstarted` | no checkout yet |
| `idle` | a checkout, nothing running, not at an interactive step |
| `active` | at an interactive step, in its Task Session |
| `moving` | a run carries it, whatever the auto setting |
| `done` · `abandoned` | completed, dropped |

Builtins are `feature` (explore, then plan, build, ship-review, deliver, verify), `code`
(the same without explore), `auto` (pursue then deliver, no interactive step: request to merged PR
without you) and `research` (findings, no PR). To change one, copy it, commit it,
and select it for a Wave with `graph: feature` in `.lf/wave/product/config.yaml`:

```bash
lf graph customize feature                # copies the builtin to .lf/graphs/feature.yaml
lf --wave product set wave.graph feature  # or only for you, without committing
```

Commit those files; teammates run your definition once they have your commit.

A Task takes up its Graph on its first `lf task run`: the one you name, then
your `lf set wave.graph` for its Wave and then its repository, then its Wave's
`graph:`, then the repository's in `.lf/config.yaml`, then builtin `code`. It keeps
that Graph as it was; later edits apply to Tasks that take it up afterwards.

## Run Tasks on other machines

```bash
lf task run LOO-12 --pool lab --rent code --quote # where it would run, and the most it could cost
lf task run LOO-12 --pool lab --rent code         # a free lab machine, else rent one
```

The Task's Graph runs on a free machine from your `lab` pool, or on one rented
from the `code` on-demand when none is free. When it ends, its Sessions and
Graph runs are copied back here before a rented machine is given back. Adding
machines, pools and on-demands is under [Machines](ops.md#machines).

## Goals

A Wave's `GOAL.md` is the prompt its conversation works from:

```markdown
<!-- .lf/wave/infra/GOAL.md -->
## Objective

Keep the runtime architecture legible. Each pass: read the Wave's Tasks and
memory, pick the next useful move, and fold what shipped into memory.

## Process

Make mechanical changes directly; write a scratch design first when the
change crosses storage, auth or public APIs.
```

**Objective** is what the Wave is for and how it moves. **Process** is
constraint: when to design first, what never to touch. KRs live in the plan
([Waves](work.md#waves)). To keep it moving without you, run an ordinary
command from your own cron or launchd:

```bash
# 0 9 * * *  cd ~/src/product && lf --wave product -b skill wave/operate
```

Next: [Troubleshooting](troubleshooting.md), what survives and what to run when something goes wrong.

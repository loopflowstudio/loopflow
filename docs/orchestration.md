# Orchestration

You've launched a Skill and a Graph. This page makes them yours: which agent
runs each step, which machine runs the Graph, how a Task moves from idea to
landed PR, and how a Wave keeps going.

Loopflow ships reasonable workflows for software engineering, and they are
templates. Copy one with `lf graph customize pursue`, change it, or write down
the way you work and share it.

## Skills

```bash
lf skill customize pr-message    # copies the builtin to .lf/skills/pr-message.md and prints the path
lf audit focus on the token refresh path   # your .lf/skills/audit.md, with a message
lf -a codex audit                # the same Skill on Codex
```

A repository Skill replaces the builtin of the same name, for everyone who has
your commit. Customize copies once, then keeps printing the existing file.

One Skill, one job: `design` writes the plan, `implement` builds it, `gate`
judges whether it ships. Chain them in a Graph rather than writing one Skill
that does everything. Pass work between steps through what already holds it:
commits, and notes in `scratch/`.

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
name another for that occurrence. `-a` overrides them all, including nested
Graphs and child `lf` commands. Without any, your configured `agent:` applies,
then the Skill's `default_agent:`, then whichever harness is installed.

```markdown
---
docs: [scripts/testing.md]
---
Run the affected suites once for this tree.
```

A repository Skill's `docs:` adds files to its context, resolved like `--docs`.

## Run a Graph anywhere

```bash
lf -b graph ship-api                     # headless, step to step; returns when it ends
lf -i graph ship-api                     # each step opens in the agent; exit it to advance
lf --machine mini -b graph ship-api      # the whole Graph on another machine you added
lf graph ship-api --context --json       # what the first step would receive; launches nothing
```

Without `-i`, every step runs headless, even from a terminal. A failed step
stops the Graph. A stopped Graph is history: inspect it with
`lf graph show ID --processes`, then start fresh work. Nothing resumes it.

Mechanical steps ride along as commands:

```yaml
# .lf/graphs/deliver.yaml (the builtin)
- gate
- cmd: land --wait-and-fix    # merge, repairing CI until it lands
```

## Loops and branches

`pursue` is a Graph with a backward edge:

```yaml
- implement
- compress
- graph: refresh      # sync with main, then realign the plan
- loop: implement     # loop-or-next decides: advance, iterate, or blocked
- concept-review
- loop: implement
- pr-publish
```

At each `loop:`, the deciding step reads the work and the plan and returns
advance, iterate (back to the target, with direction) or blocked (with the
question). There is no pass limit; iterate repeats as long as there's useful
work. Name your own decider with `step:`:

```yaml
- step: {name: implement, id: first}
- implement
- loop: first
  step: my-decider
```

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

## Workflows

A Task's Workflow is a graph whose `interactive:` steps are stages: places
where you take part in the Task's conversation. Everything between stages runs
on its own.

```yaml
# feature (builtin)
- kickoff
- loop: kickoff
  interactive: explore      # you choose what to build; repeat or go on
- graph: pursue
- loop: implement
  interactive: demo         # you try the change
- graph: deliver            # gate, then land and stay until it merges
- loop: implement
  interactive: verify       # you check it on main; fix forward or finish
```

```bash
lf task run LOO-12              # start: runs kickoff up to explore
lf task run LOO-12 pursue       # at explore: build it, up to demo
lf ship LOO-12                  # every remaining step to merge, standing in at each stage
lf task workflow move LOO-12 explore --reason "rethink the API"   # go back, running nothing
```

Routes between stages are named by what they run first (the composed graph,
else the skill): `pursue` and `deliver` above, and `end` for a route that runs
nothing. A run moves the Task onto the route; success puts it at the next stage.
A failed or stopped Graph leaves it on the route: run it again, take another,
or `move` it. `lf task status LOO-12` shows the stage, the routes you can take
now and every move with who made it. There is no approve command; give feedback
in the Task's conversation, which knows its stage and the command for each route.

`lf ship` returns once the Task is shipping; a headless agent stands in at each
remaining stage, decides advance or repeat, and records why in the Task's
history. `lf ship --stop` withdraws it. Any route into `end` ships the same way.

| Task state | When |
|---|---|
| `unstarted` | no checkout yet |
| `idle` | a checkout, no Graph running, not at a stage |
| `active` | at a stage, in its Task Session |
| `moving` | a Graph runs, with a stage ahead that stops for you |
| `shipping` | a Graph runs and no stage ahead stops it |
| `deferred` · `done` · `abandoned` | held, completed, dropped |

Builtins are `feature` (explore, demo, deliver, verify), `code` (demo, deliver,
verify), `auto` (pursue then deliver, no stops: request to merged PR without
you) and `research` (findings, no PR). To change one, copy it, commit it, and
select it for a Wave:

```bash
lf graph customize feature               # copies the builtin to .lf/graphs/feature.yaml
lf wave workflow set feature product     # future product Tasks take it up
```

A Task captures its Workflow on its first `lf task run`: the one you name, then
its Wave's default, then repository `workflow:` in `.lf/config.yaml`, then
builtin `code`. Later edits apply to new Tasks; `lf task workflow restart LOO-12`
moves an existing one back to `start` without reloading it.

## Run Tasks on other machines

```bash
lf task run LOO-12 --pool lab --rent code --quote # where it would run, and the most it could cost
lf task run LOO-12 --pool lab --rent code         # a free lab machine, else rent one
```

The Task's Graph runs on a free machine from your `lab` pool, or on one rented
from the `code` on-demand when none is free. When it ends, its Sessions and
Processes are copied back here before a rented machine is given back. Adding
machines, pools and on-demands is on [Ops](ops.md).

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

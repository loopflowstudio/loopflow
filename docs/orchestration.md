# Orchestration

You've launched a Skill and a Graph. This page makes them yours: which agent
runs each step, which machine runs the Graph, how a Task moves from idea to
landed PR, and how a Wave keeps going.

Loopflow ships reasonable Graphs for software engineering, and they are
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
lf -i graph ship-api                     # every step opens in the agent; exit it to advance
lf --machine mini -b graph ship-api      # the whole Graph on another machine you added
lf graph ship-api --context --json       # what the first step would receive; launches nothing
```

Without `-i` or `-b`, steps run headless except `interactive:` ones, which open
here ([below](#where-a-person-takes-part)). A failed step stops the Graph. A
stopped Graph is history: inspect it with `lf graph show ID --processes`, then
start fresh work. Nothing resumes it.

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

## Where a person takes part

A step written `interactive: <skill>` is where a person takes part; every other
step runs headless. A Task's workflow is the Graph it took up and the step it
stands at.

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
lf graph feature                    # explore and demo open here with you; the rest runs on its own
lf -b graph feature                 # every step headless; an agent stands in at explore and demo
lf task run LOO-12 feature          # runs up to explore and stops; the Task waits in its conversation
lf task run LOO-12                  # after you're done there: on to demo
lf task run LOO-12 --from kickoff   # go back a round
lf -b task run LOO-12               # the rest, straight through to a merged PR
lf ship LOO-12                      # the same, returning once the Task is shipping
```

`-i` makes every step of a launch interactive and `-b` every step headless. At an
interactive step with a loop behind it, you, or the agent standing in, decide
advance or iterate; blocked there, the Task waits at that step. A run that
otherwise stops or fails leaves the Task where that run began; the next
`lf task run` starts it again. `--from` takes a step's id, its Skill, or the
Graph it was composed from; starting from an interactive step only puts the Task
there. After the last step the Task reaches `end`, which requests completion.
`lf ship` or any run into `end` hands the Task to Auto-Ship, which may land its
PR. Other Graphs in the Task's worktree never move it. There is no approve command: give feedback in the
Task's conversation, which knows its step and how to go on. `lf ship --stop`
withdraws Auto-Ship.

| Task state | When |
|---|---|
| `unstarted` | no checkout yet |
| `idle` | a checkout, nothing running, not at an interactive step |
| `active` | at an interactive step, in its Task Session |
| `moving` | a run carries it, with an interactive step ahead |
| `shipping` | a run carries it and nothing ahead stops it |
| `deferred` · `done` · `abandoned` | held, completed, dropped |

Builtins are `feature` (explore, demo, deliver, verify), `code` (demo, deliver,
verify), `auto` (pursue then deliver, no interactive step: request to merged PR
without you) and `research` (findings, no PR). To change one, copy it, commit it,
and select it for a Wave with `graph: feature` in `.lf/wave/product/config.yaml`:

```bash
lf graph customize feature                # copies the builtin to .lf/graphs/feature.yaml
lf --wave product set graph feature       # or only for you, without committing
```

Commit those files; teammates run your definition once they have your commit.

A Task takes up its Graph on its first `lf task run`: the one you name, then
your `lf set graph` for its Wave and then its repository, then its Wave's
`graph:`, then the repository's in `.lf/config.yaml`, then builtin `code`. It keeps
that Graph as it was; later edits apply to Tasks that take it up afterwards.

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

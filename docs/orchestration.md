# Orchestration

You've launched a Skill and a Graph. This page makes them yours: which agent
runs each step, which machine runs the Graph, how a Task moves from idea to
landed PR, and how a Wave keeps going.

Loopflow ships reasonable Graphs for software engineering, and they are
templates. Copy one with `lf graph customize feature`, change it, or write down
the way you work and share it.

## Skills

A Skill is one job in Markdown: `design` plans, `implement` builds, `qa`
verifies, `gate` reviews and publishes. Chain Skills in a Graph rather than
cramming everything into one prompt; steps pass work through Git commits and
notes in `scratch/`.

### Extend or replace a builtin

```bash
lf skill customize pr-message    # creates .lf/skills/pr-message.md extending the builtin
lf skill customize gate --full   # copies the full builtin text to replace it
lf list skill                    # shows which Skills extend vs. replace builtins
```

Extending a builtin lets you add repo-specific instructions while still
receiving upstream improvements to the builtin:

```markdown
<!-- .lf/skills/gate.md -->
---
extends: builtin
---
## In this repository

Run `make e2e` before publishing.
```

A Skill without `extends: builtin` replaces the builtin of the same name for
everyone on your team.

### Automatic commits per step

When a Skill step edits files in a Graph, Loopflow commits them automatically
(and pushes on non-default branches):

```text
lf implement: Count retries per Task

The board read attempts from the wrong row.
```

The `commit-message` Skill writes commit messages (and `pr-message` writes PR
copy). Route them to a fast, cheap model:

```bash
lf set --global skill.commit-message.default_agent claude:haiku
```

Using `default_agent` keeps your `agents` failover list behind it if Haiku is
unavailable. Run the same commit helper manually with `lf commit --step implement`.

## Pick the agent per Skill or Graph step

Specify `agent:` (pinned) or `default_agent:` (with failover) in Skill
frontmatter, Graph steps, or config:

```markdown
<!-- .lf/skills/audit.md -->
---
agent: codex
context:
  docs: [scripts/testing.md]
---
Audit auth changes on this branch. Fix missing validation and untested paths.
```

```yaml
# .lf/graphs/ship-api.yaml
- step: {name: implement, agent: claude:opus}
- audit                         # runs on Codex per its frontmatter
- gate
```

- `agent:` pins the harness (and model, if named).
- `default_agent:` sets the preferred starting point while letting headless
  steps fail over through your [`agents`](config.md#keep-going-when-a-provider-runs-out)
  list on usage limits or unavailable models.
- Passing `-a <harness>` to `lf graph` overrides every step in that Graph run
  (including nested Graphs and routers) without affecting unrelated child Tasks.
- `context.docs` in Skill frontmatter attaches extra files whenever that Skill
  runs.

See [Configuration](config.md) for the full precedence rules across personal,
repository, Wave, Skill, and Graph-step settings.

## Compose Graphs, loops, and branches

Run any Graph directly, headless, interactively, or on another Machine:

```bash
lf -b graph ship-api                     # headless end to end
lf -i graph ship-api                     # every step interactive
lf --machine mini -b graph ship-api      # run on a remote Machine
lf graph ship-api --context --json       # preview first step's context without launching
```

Mix Skills and mechanical `cmd:` steps:

```yaml
# deliver (builtin)
- reflect                     # folds your feedback from this Task into skill edits
- gate                        # publishes PR so CI starts, then reviews while CI runs
- cmd: land --wait-and-fix    # merges when checks pass, repairing CI failures
```

### Loops (`loop:`)

A `loop:` block repeats as long as its final step says `iterate`. The built-in
`plan` and `build` Graphs compose two nested loops:

```yaml
# plan (builtin)
- kickoff              # plan the change, answering key risks
- concept-review       # pressure-test the premise before writing code
- write-docs           # update actual user docs before implementation

# build (builtin)
- loop:
    - loop:
        - implement        # build one reviewable chunk
        - cmd: sync
        - realign          # decides: next chunk (iterate) or move on (advance)
    - compress             # simplify what the branch touched
    - qa                   # exercise the real change end to end
    - concept-review       # decides: ship whole change or loop back
- pr-publish
```

Any Skill can be the deciding step of a loop: it returns `advance`, `iterate`
(with direction), or `blocked` (with a question). There is no arbitrary pass
limit; loops continue while useful work remains.

### Docs-first (`write-docs`) and QA playbooks (`.lf/qa/`)

- **`write-docs`** updates your actual README, guides, or reference docs right
  after planning so you can read and edit what is being built while
  implementation runs (in Loopflow Desktop, the edited files open in the Files
  pane automatically; elsewhere, their paths are printed in the conversation).
- **`qa`** maintains repo-local verification playbooks under `.lf/qa/`, one
  file per surface. The first line of each file states when to read it, so runs
  open only the playbooks relevant to their diff, and update them when a
  surface changes.

### Conditional routing (`xor:`)

`xor:` picks exactly one branch based on a router Skill's `{"path": "NAME"}`
output (an empty branch exits cleanly):

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

### Optimization loops (`optimize`)

```bash
lf graph optimize : "cut p50 export time on the 60k-row dataset"
```

`optimize` measures a baseline, plans one change, implements it, and keeps or
reverts based on the measurement until no clear improvement remains, then
compresses and re-measures. Compose it with `deliver` to ship the win:

```yaml
# .lf/graphs/speed-up.yaml
- graph: optimize
- graph: deliver
```

## Where a person takes part

Mark any step `interactive:` to pause for human input; every other step runs
headless. Built-in `feature` puts you in the loop at three high-leverage
moments:

```yaml
# feature (builtin)
- interactive: explore      # choose what to build
- graph: plan               # kickoff, concept-review, write-docs
- loop:
    - loop:
        - graph: build      # implement, compress, qa, publish PR
        - interactive: ship-review  # read one summary page; iterate sends feedback back to build
    - graph: deliver        # reflect, gate, land --wait-and-fix
    - interactive: verify   # check it on main; finish or fix forward
```

```bash
lf task run LOO-12 feature          # stops at explore; opens in the Task conversation
lf task run LOO-12                  # continues from where it stopped
lf task run LOO-12 --from explore   # jump back to any step (by id, Skill, or sub-Graph)
lf -b task run LOO-12               # run headless through all remaining steps
lf task auto on LOO-12              # auto on: an agent stands in for you at every remaining step
```

At **`ship-review`**, Loopflow builds one HTML page (`scratch/ship-review.html`):
what ships, in your terms, and everything the run left outside the PR, each
with a proposal to fix it here, give it its own Task, or drop it. It is not a
code review; concept review and gate already answered whether the change is
right. Ask for changes in the Task conversation to loop back to `build`, or
move on to `deliver`.

### Built-in Graphs

| Graph | What it does |
|---|---|
| `feature` | `explore` (interactive) → `plan` → `build` ↔ `ship-review` (interactive) → `deliver` → `verify` (interactive) |
| `code` | Same as `feature`, starting at `plan` (default when no Graph is configured) |
| `pursue` | `plan` then `build` with no interactive stops: request to published PR |
| `research` | Investigate and report findings without opening a PR |
| `optimize` | Iterative measure → change → keep/revert loop |

Copy any builtin to customize it for your team:

```bash
lf graph customize feature                # writes .lf/graphs/feature.yaml
lf --wave product set wave.graph feature  # personal override without committing
```

### Describe a Graph

```yaml
# .lf/graphs/ship-api.yaml
description: Build the endpoint, audit it, then deliver.
steps:
- implement
- audit
- interactive: demo
- graph: deliver
```

```console
$ lf graph list
code       Build, deliver and verify. You review the change, then check it on main.
my-graph
ship-api   Build the endpoint, audit it, then deliver.
```

`description:` is one line saying what running the Graph gets you. Desktop's
Start menu shows it beside the Graph's name, with a stop count taken from its
`interactive:` steps. Every builtin has one. A Graph without it, written as a
bare list of steps, still runs and still lists; Start offers it under
**More ›**.

### Auto

Auto is a setting on the Task: on, off, or never set. While it is on, the
Task's runs stand in for you at each interactive step they reach and may merge
the PR. Turning it on runs the Task: `lf task auto on` and Desktop's **Auto**
toggle are one operation.

```bash
lf task auto on LOO-12              # LOO-12 · code from ship-review running as Graph run …
lf task status LOO-12               # LOO-12  moving · auto: on by Maya, Nov 18 09:04
lf task auto off LOO-12             # LOO-12: auto off
```

- The run reads auto at each interactive step, not at launch: turn it on while
  `build` runs and that run stands in at `ship-review` when it gets there.
- `lf task auto off` kills nothing. The run waits at its next interactive step,
  or before a merging step (`lf land`, `lf pr arm`, `pr-land`): "Waiting at pr
  land: auto is off (Maya). `lf task auto on INF-1` lands it." Auto-merge that
  Loopflow armed is disarmed.
- Off stands: only `lf task auto on` turns it back on; an `auto: on` in
  configuration never does. `lf -b task run` stands in for its own run and
  writes no auto, so with auto off it still waits at the landing step.
- Never set, a run that reaches a merging step lands; `lf task run` says so on
  its `merges:` line. A failing run leaves auto alone.
- `auto: on` beside `graph:` in a Wave's config turns auto on for every Task
  that takes the Graph up. Reopening a Task clears it.

### Failure and retries

- A failed step is tried again, up to three attempts, starting at that step (earlier steps stay done).
  Pre-launch failures (such as a missing harness CLI) fail immediately so you
  can fix them and run `lf task run`.
- If a headless OpenCode or Pi agent asks a question, the run stops and prints
  it; answer with `lf steer LOO-12 "…"` and continue with `lf task run LOO-12`.

| Task state | Meaning |
|---|---|
| `unstarted` | No worktree yet |
| `idle` | Checked out, nothing running, not at an interactive step |
| `active` | Waiting at an interactive step in its Task conversation |
| `moving` | Graph running, whatever the auto setting |
| `done` / `abandoned` | Completed / dropped |

## Run Tasks on pools or rented machines

```bash
lf task run LOO-12 --pool lab --rent code --quote   # preview placement and max cost
lf task run LOO-12 --pool lab --rent code           # run on a free `lab` box or rent from `code`
```

When the run ends, its Sessions and Graph history copy back to your machine
before any rented box is returned. See [Machines](ops.md#machines).

## Wave Goals

A Wave's `.lf/wave/<name>/GOAL.md` is the operating brief for its conversation
and Tasks:

```markdown
<!-- .lf/wave/infra/GOAL.md -->
## Objective

Keep the runtime architecture legible. Each pass: read the Wave's Tasks and
memory, pick the next useful move, and fold what shipped into memory.

## Process

Make mechanical changes directly; write a scratch design first when the
change crosses storage, auth, or public APIs.
```

Next: [Troubleshooting](troubleshooting.md): what survives crashes and how to
diagnose anything that stops.

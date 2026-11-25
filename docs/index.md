# Loopflow

```bash
curl -fsSL https://loopflow.studio/install.sh | sh
lf init
npx skills add loopflowstudio/loopflow --skill loopflow -g   # teach your coding agent to use lf
```

`lf` is how your agent orchestra conducts itself. It's a task system for coding
agents: you say what you want, and each Task gets its own conversation, its own
workflow and its own pull request. Organize Tasks into Waves to give each area of
work its own defaults. It works with Claude Code, Codex, OpenCode, Pi and Antigravity, on your
existing subscriptions, inside whatever terminal, multiplexer and tracker you
already use.

**Your agent can drive it.** The last install line teaches your own coding agent
to use `lf`, so you can just ask it: *"Split the auth cleanup into Tasks and run
them with lf."* [What the skill teaches](https://github.com/loopflowstudio/loopflow/blob/main/skills/loopflow/README.md).

Run Tasks in parallel, each in its own worktree and branch. Nothing is hosted:
conversations stay on your machine, and only the plan travels, through a Git
ref by default or two-way with Linear or Asana.

**1. A Task is a conversation with a workflow behind it.**

```bash
lf task create "fix the flaky retry test" --run       # files it, starts its workflow, opens its conversation
lf -b task create "fix the flaky retry test" --run    # straight to a merged PR, no stops
lf resume --task <ID>                                 # pick up its conversation from any terminal
```

Each Task carries:

- **One conversation.** A persistent chat thread with your agent where everything
  about the Task goes: you talk in it, reviews happen in it, results come back to
  it. Open it in several terminals or Desktop at once; it outlives closed
  windows, `lf install` updates, crashes and reboots.
- **A workflow (Graph).** Steps like `design`, `implement`, `qa`, `ship-review`,
  and `deliver`, each running in the Task's own worktree and branch. `lf task run`
  pauses at `interactive:` steps for your input and runs the rest headless;
  `lf -b task run` or `lf task auto on` runs every step autonomously.
- **Its own pull request, branch and tracker issue,** kept together, through
  stacked Tasks (`lf sync <parent-ID>`) and CI repair (`lf land --wait-and-fix`).
- **Its history.** Every conversation, Graph run and step, and what each cost.

```bash
lf steer <ID> "keep the retry budget at 3"   # redirects running work at its next turn or step
lf task status <ID>                          # where it stands and what it needs from you
lf session usage --task <ID>                 # tokens and cost across every run
lf land --wait-and-fix                       # merge when CI passes; repair failures if not
```

**2. Write down how work runs.** A Task's workflow is a Graph: a YAML list of
Skills, and a Skill is a Markdown file, both in your repository and reviewed like
code. Each step can name its own agent, and `agents: [claude, codex]` moves
headless work to the next one when a provider hits its usage limit.

```markdown
<!-- .lf/skills/audit.md -->
---
agent: codex
---
Audit auth changes on this branch. Fix missing validation and untested paths.
```

```yaml
# .lf/graphs/ship-api.yaml
- interactive: design   # stops for you; writes scratch/plan.md
- implement             # commits its work on the branch
- audit                 # runs on Codex per its frontmatter
- gate                  # publishes the PR so CI starts, then reviews while CI runs
```

```bash
lf audit                        # run one Skill right now
lf graph ship-api               # design with you, then run the rest headless
lf task run <ID> ship-api       # run a Task through it, pausing at design
lf -b task run <ID> ship-api    # run every step headless to a merged PR
```

Built-in Graphs (`feature`, `code`, `quick`, `research`, `optimize`) work out of
the box and are templates you can customize (`lf graph customize feature`).
Extend built-in Skills with `extends: builtin` instead of forking them.

**3. Organize Tasks into Waves.** A Wave is an area of work with its own goal
(`GOAL.md`), memory (`MEMORY.md`) and plan, and its own setup for every Task in
it: the Graph, the agent, and the machine where it runs. Your repository is the
Wave for Tasks without one.

```yaml
# .lf/wave/infrastructure/config.yaml (committed)
graph: code
auto: on              # infrastructure Tasks go from request to merged PR
agent: claude:opus
```

```bash
lf --wave infrastructure set machine mini    # run your infra Tasks on a remote box
lf wave status infrastructure                # every setting and where it came from
```

**Without a Task,** `lf` still launches any agent or Skill directly:

```bash
lf : "fix the flaky retry test"            # launch your default agent
lf -a codex : "fix the flaky retry test"   # run the same prompt on Codex
lf debug -c                                # paste an error from clipboard, find and fix the cause
lf resume                                  # return to this checkout's conversation
```

## What makes Loopflow different

- **It runs on your machine.** Agents run on your laptop, your own SSH hosts,
  or a machine you rent for one Task. Loopflow hosts nothing and bills nothing:
  each agent uses its own Claude Code, Codex, OpenCode, Pi or Antigravity login, and talks
  to its provider directly.
- **Any agent, per step.** Send design to Claude, implementation to Codex and
  commit messages to Haiku. When a provider hits its usage limit, headless
  steps move to the next agent in your `agents` list, or wait and try again.
- **Workflows are files in your repository.** Skills are Markdown and Graphs
  are YAML under `.lf/`, reviewed like code. A Graph can loop
  (`sync → implement` inside `concept-review → realign`), branch (`xor`), write the docs before the
  code (`write-docs`), keep QA playbooks (`.lf/qa/`), and stop for you with
  one summary page (`ship-review`).
- **Your team keeps its tools.** Teammates review PRs on GitHub and comment in
  Linear or Asana without installing `lf`; their comments reach running agents
  as steers. With no tracker, the plan travels with the repository through Git.
- **Any window.** Use Loopflow Desktop on macOS (and your phone, over
  Tailscale), or stay in Rex, cmux, Herdr, tmux or a plain terminal. They all
  read the same local store and the same `--subscribe --json` streams.

## Navigate by use case

| You want to know about… | Read |
|---|---|
| Running your first Task from prompt to merged PR | [Get started](getting-started.md) |
| Parallel Tasks, steering, stacking PRs, and team sync (Git, Linear, Asana) | [Work](work.md) |
| Persistent conversations, multi-terminal views, and crash/reboot recovery | [Sessions](sessions.md) |
| Mixing Claude, Codex, OpenCode, Pi, and Antigravity + automatic usage-limit failover | [Configuration](config.md#keep-going-when-a-provider-runs-out) · [Native authentication](subscriptions.md) |
| Custom Skills (`extends: builtin`), Graphs, loops, `ship-review`, and `qa` | [Orchestration](orchestration.md) |
| Auto-merge, CI repair (`lf land --wait-and-fix`), stacked PRs, and releases | [Ops](ops.md) |
| Running on SSH machines, pools, or rented Daytona/Modal/RunPod/AWS boxes | [Machines](ops.md#machines) · [Sign in on any Machine](subscriptions.md#sign-in-on-any-machine) |
| Loopflow Desktop, Rex, cmux, Herdr, tmux, or building a custom UI | [Surfaces](surfaces.md) · [Workspaces](workspaces.md) |
| Token cost, context budgets, transcripts, and `lf doctor` | [Telemetry](telemetry.md) |
| Letting another AI agent orchestrate `lf` via `--json` | [The Agent API](agent-api.md) |
| Execution boundaries, sandboxes, and credential handling | [Security](security.md) |

## Read in order

Each page picks up where the previous one leaves off:

| Area | Page | Covers |
|---|---|---|
| Start | [Get started](getting-started.md) | Install, first Skill, first Task, review, and ship |
| Work | [Work](work.md) | Multiple Tasks, steering, shared plans (Git/Linear/Asana), Waves |
| Sessions | [Sessions](sessions.md) | Multi-view attach, `lf resume`, update/crash survival, switching agents |
| Ops | [Ops](ops.md) | PRs, `gate`, CI repair, releases, checkouts/stacking, SSH & rented machines |
| Surfaces | [Surfaces](surfaces.md) · [Workspaces](workspaces.md) | Desktop, Rex, cmux, Herdr, tmux, `lf open`, `--subscribe --json` |
| Client server | [Connect clients](server.md) | Local server lifecycle, Desktop reconnect, phone access over Tailscale |
| Telemetry | [Telemetry](telemetry.md) | Cost, context budgets, transcripts, live runs, `lf doctor` |
| Orchestration | [Orchestration](orchestration.md) | Custom Skills, Graphs, loops, `xor`, `optimize`, `ship-review`, Goals |
| Agent API | [The Agent API](agent-api.md) | Driving `lf` from another agent: delegate, steer, ship, observe |
| Reference | [Configuration](config.md) · [Native authentication](subscriptions.md) · [Security](security.md) · [Glossary](glossary.md) · [`lf` reference](lf-reference.md) · [Troubleshooting](troubleshooting.md) | Exact settings, auth, trust boundaries, terms, CLI flags, fixes |

## Where files live

```text
.lf/                      # Committed repo config and extensions
  config.yaml             # Default graph, tracker, context, release targets
  config.local.yaml       # Your private repo/Wave preferences (auto-excluded from Git)
  skills/                 # Repo Skills (extend or replace builtins)
  graphs/                 # Repo Graphs
  qa/                     # Repo QA playbooks, updated by `qa`
  wave/<name>/            # Wave GOAL.md, MEMORY.md, and config.yaml
scratch/                  # Temporary working notes (added files cleared on submit/land)
~/.lf/                    # Global config, skills, and local SQLite stores
  loopflow.db             # Active state: Tasks, Sessions, running Processes
  history.db              # Turns (kept) and Session history (90 days)
```

## Context at a glance

Providers load `AGENTS.md` natively; Loopflow adds `LOOPFLOW.md` (the operating
contract), `scratch/`, and `.lf/wave/` automatically. Pull in more context when
you need it:

```bash
lf gate --docs DESIGN.md            # one file
lf gate --docs 'docs/*.md'          # a glob
lf gate --diff files                # paths of files changed on the branch
lf debug -c                         # clipboard contents
lf --context debug -c               # preview assembled input without launching
```

## For agents

Every docs page is available as raw Markdown: append `.md` to any URL
(`/docs/work.md`) or send `Accept: text/markdown` to the canonical URL. The
curated index is [/llms.txt](/llms.txt); the full corpus in one file is
[/llms-full.txt](/llms-full.txt).

## Next

[Get started →](getting-started.md) · [Work →](work.md) · [The Agent API →](agent-api.md)

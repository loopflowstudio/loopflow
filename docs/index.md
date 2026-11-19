# Loopflow

```bash
curl -fsSL https://loopflow.studio/install.sh | sh
lf init
npx skills add loopflowstudio/loopflow --skill loopflow -g   # teach your coding agent to use lf
```

Loopflow (`lf`) is a local-first task and workflow system for coding agents.
You describe the outcome you want; each Task gets its own Git worktree, branch,
persistent conversation, workflow, and pull request. Organize Tasks into Waves
to give each area of your codebase its own goal, memory, and defaults.

It runs on **Claude Code, Codex, OpenCode, and Pi**, using your existing native
logins, inside whatever terminal, multiplexer, or tracker you already use.
Nothing is hosted: conversations and code stay on your machine, and only the
plan syncs (through a Git ref by default, or two-way with Linear or Asana).

**Your coding agent can drive it.** The third install line teaches Claude Code,
Codex, OpenCode, or Pi to operate `lf`, so you can stay in your main
conversation and say: *"Split the auth cleanup into Tasks and run them with
lf."* [What the skill teaches](https://github.com/loopflowstudio/loopflow/blob/main/skills/loopflow/README.md).

**1. Every Task is a durable conversation backed by a workflow.**

```bash
lf task create "fix the flaky retry test" --run                    # files it, starts its workflow, opens its conversation
lf -b task create "fix the flaky retry test" --run --graph auto    # straight to a merged PR, no stops
lf resume --task <ID>                                              # pick up its conversation from any terminal
```

Each Task carries:

- **One persistent conversation.** Talk to the agent, review its work, and get
  status updates in one thread. Open it in multiple terminals or Desktop at
  once; it survives closing windows, `lf install` updates, crashes, and reboots.
- **A workflow (Graph).** Steps like `design`, `implement`, `qa`, `human-review`,
  and `deliver`, each running in the Task's own worktree and branch. `lf task run`
  pauses at `interactive:` steps for your input and runs the rest headless;
  `lf -b task run` or `lf ship` runs every step autonomously.
- **One branch, worktree, PR, and tracker issue,** kept in sync — including
  stacked Tasks (`lf sync <parent-ID>`) and automatic CI repair
  (`lf land --wait-and-fix`).
- **Full history and cost.** Every turn, token count, context breakdown, and
  Graph step recorded locally.

```bash
lf steer <ID> "keep the retry budget at 3"   # redirects running work at its next turn or step
lf task status <ID>                          # where it stands and what it needs from you
lf session usage --task <ID>                 # tokens and cost across every run
lf land --wait-and-fix                       # merge when CI passes; repair failures if not
```

**2. Workflows are plain Markdown and YAML in your repo.** A Graph is a YAML
list of Skills, and a Skill is a Markdown file — committed, reviewed like code,
and shared with your team. Mix harnesses per step and configure automatic
usage-limit failover (`agents: [claude, codex]`).

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

Built-in Graphs (`feature`, `code`, `auto`, `research`, `optimize`) work out of
the box and are templates you can customize (`lf graph customize feature`).
Extend built-in Skills with `extends: builtin` instead of forking them.

**3. Waves give ongoing areas of work their own memory and defaults.** A Wave
holds a lasting objective (`GOAL.md`), curated memory (`MEMORY.md`), KRs, and
defaults (Graph, agent, machine) inherited by every Task in it. Your repository
is the default scope for Tasks outside a Wave.

```yaml
# .lf/wave/infrastructure/config.yaml (committed)
graph: auto           # infrastructure Tasks go from request to merged PR
agent: claude:opus
```

```bash
lf --wave infrastructure set machine mini    # run your infra Tasks on a remote box
lf wave status infrastructure                # every setting and where it came from
```

**No Task required.** Use `lf` directly to launch any harness or Skill in your
current terminal:

```bash
lf : "fix the flaky retry test"            # launch your default agent
lf -a codex : "fix the flaky retry test"   # run the same prompt on Codex
lf debug -c                                # paste an error from clipboard, find and fix the cause
lf resume                                  # return to this checkout's conversation
```

## What makes Loopflow different

- **Local-first, zero hosted control plane.** Everything runs on your machine
  (or your own SSH hosts and on-demand rentals). You keep your native Claude
  Code, Codex, OpenCode, and Pi logins — no proxy billing, no cloud lock-in,
  and transcripts never leave your machine.
- **Multi-harness with automatic failover.** Route design to Claude,
  implementation to Codex, and commit messages to Haiku. When a provider hits a
  usage limit, headless steps automatically move to the next agent in your
  `agents` list or wait and retry.
- **Version-controlled workflows.** Skills (Markdown) and Graphs (YAML) live in
  `.lf/` and review like code. Compose loops (`implement → sync → realign`),
  conditional branches (`xor`), docs-first authoring (`write-docs`), repo-local
  QA playbooks (`.lf/qa/`), and one-page `human-review` summaries.
- **Built for small teams without forcing a tool switch.** Teammates review PRs
  on GitHub and comment in Linear or Asana without installing `lf`; their
  comments arrive as steers to running agents. Without a tracker, plans sync
  over a Git ref on `origin`.
- **Surface-agnostic.** Use Loopflow Desktop on macOS (plus phone access over
  Tailscale), or stay in Rex, cmux, Herdr, tmux, or a plain terminal. Every
  surface reads the same local store and `--subscribe --json` streams.

## Navigate by use case

Jump straight to what matters for your setup:

| If you are evaluating… | Start here |
|---|---|
| Running your first Task from prompt to merged PR | [Get started](getting-started.md) |
| Parallel Tasks, steering, stacking PRs, and team sync (Git, Linear, Asana) | [Work](work.md) |
| Persistent conversations, multi-terminal views, and crash/reboot recovery | [Sessions](sessions.md) |
| Mixing Claude, Codex, OpenCode, and Pi + automatic usage-limit failover | [Configuration](config.md#keep-going-when-a-provider-runs-out) · [Native authentication](subscriptions.md) |
| Custom Skills (`extends: builtin`), Graphs, loops, `human-review`, and `qa` | [Orchestration](orchestration.md) |
| Auto-merge, CI repair (`lf land --wait-and-fix`), stacked PRs, and releases | [Ops](ops.md) |
| Running on SSH machines, pools, or rented Daytona/Modal/RunPod/AWS boxes | [Machines](ops.md#machines) · [Remote forwarding](subscriptions.md#remote-machines-and-rentals) |
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
| Orchestration | [Orchestration](orchestration.md) | Custom Skills, Graphs, loops, `xor`, `optimize`, `human-review`, Goals |
| Agent API | [The Agent API](agent-api.md) | Driving `lf` from another agent: delegate, steer, ship, observe |
| Reference | [Configuration](config.md) · [Native authentication](subscriptions.md) · [Security](security.md) · [Glossary](glossary.md) · [`lf` reference](lf-reference.md) · [Troubleshooting](troubleshooting.md) | Exact settings, auth, trust boundaries, terms, CLI flags, fixes |

## Where files live

```text
.lf/                      # Committed repo config and extensions
  config.yaml             # Default graph, tracker, context, release targets
  skills/                 # Repo Skills (extend or replace builtins)
  graphs/                 # Repo Graphs
  qa/                     # Repo QA playbooks, updated by `qa`
  wave/<name>/            # Wave GOAL.md, MEMORY.md, and config.yaml
lf.config.local.yaml      # Your private repo/Wave preferences (auto-excluded from Git)
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
lf gate --docs VISUAL_DESIGN.md     # one file
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

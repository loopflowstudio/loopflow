# Loopflow

```bash
curl -fsSL https://loopflow.studio/install.sh | sh
lf init
npx skills add loopflowstudio/loopflow --skill loopflow -g -y   # teach your coding agent to use lf
```

`lf` is how your agent orchestra conducts itself. It's a task system for coding
agents: you say what you want, and each Task gets its own conversation, its own
workflow and its own pull request. Organize Tasks into Waves to give each area of
work its own defaults. It works with Claude Code, Codex, OpenCode and Pi, on your
existing subscriptions, inside whatever terminal, multiplexer and tracker you
already use.

**1. A Task is a conversation with a workflow behind it.**

```bash
lf task create --title "fix the flaky retry test"   # prints the Task ID
lf -b task run <ID>                                   # its worktree, its agents, its pull request
lf resume --task <ID>                                 # its conversation, from any terminal
```

Each Task has:

- **One conversation.** A persistent chat thread with your agent where everything
  about the Task goes: you talk in it, reviews happen in it, results come back to
  it. One context, on whichever harness and login it runs; it outlives terminals
  and crashes.
- **A workflow.** The Graph it runs (design, build, demo, land), each step in the
  Task's own worktree and branch. `lf task run` stops at the steps where you take
  part and the rest run without you; `lf -b task run` runs every step itself.
- **Its own pull request, branch and tracker issue,** kept together.
- **Its history.** Every conversation, Graph run and step, and what each cost.

```bash
lf comment <ID> "keep the retry budget at 3"   # reaches the next step, in its context
lf task status <ID>                            # where it is in its workflow, and what it needs from you
lf history usage --task <ID>                   # what it cost
lf land --wait-and-fix                         # merge when checks pass; fix them if not
```

The last install line teaches your own coding agent to do all of this, so you
can just ask it: *"Split the auth cleanup into Tasks and run them with lf."*
[What the skill teaches](https://github.com/loopflowstudio/loopflow/blob/main/skills/loopflow/README.md).

**2. Write down how work runs.** A Task's workflow is a Graph: a YAML list of
Skills, and a Skill is a Markdown file, both in your repository and reviewed like
code. Each step can name its own agent.

```markdown
<!-- .lf/skills/audit.md -->
Audit auth changes on this branch. Fix missing validation and untested paths.
```

```yaml
# .lf/graphs/ship-api.yaml
- interactive: design   # you take part; leaves scratch/<task>.md
- implement             # leaves commits on the branch
- audit
- gate                  # leaves passing checks
```

```bash
lf audit                        # one Skill, here and now
lf graph ship-api               # design with you here, then the rest on its own
lf task run <ID> ship-api       # one Task through it; stops at design and waits for you
lf -b task run <ID> ship-api    # every step headless, design included, to a merged PR
```

A step marked `interactive:` is where a person takes part. Each step can name its
harness (`agent:`); `-i`, `-b`, `-a` and `--machine` choose for the whole run.

Loopflow ships reasonable Graphs for software engineering, and they are
templates. Copy one with `lf graph customize pursue`, change it, or write down
the way you work and share it.

**3. Organize Tasks into Waves.** A Wave is an area of work with its own goal,
memory and plan, and its own setup for every Task in it: the Graph, the agent,
the machine and the login that pays. Your repository is the Wave for Tasks
without one.

```yaml
# .lf/wave/infrastructure/config.yaml, committed: shaped like .lf/config.yaml
graph: auto           # infrastructure Tasks go from request to merged PR without you
agent: claude:opus
```

```bash
lf --wave infrastructure set machine mini                       # and run on your mini
lf --wave infrastructure set account.claude work@example.com    # billed to your work login
lf wave status infrastructure                                   # each setting and where it came from
```

**Without a Task,** `lf` still launches any agent or Skill directly:

```bash
lf : "fix the flaky retry test"            # an agent, on whichever of your logins has room
lf -a codex : "fix the flaky retry test"   # the same thing on Codex
lf debug -c                                # a Skill: paste an error, it finds and fixes the cause
lf resume --all                            # after a crash or reboot, every interrupted conversation comes back
```

## The pieces

```bash
lf -b implement
lf session list --interactive false --json
lf session connect SESSION
lf task run INF-123
```

A Skill supplies instructions; a Graph composes skills and mechanical operations.
A Task owns one change, its checkout and PRs. A Wave keeps the objective
and memory; its current Project holds the plan; the Wave names the graph its Tasks take up by default.

Process records an actual lf command process. LfSession keeps the conversation,
including headless work. A running Graph is one lf process and the step processes
it starts. A conversation can outlive its command. A Graph whose
command stopped stays as history; fresh work is launched explicitly.

Read the [contract and cutover status](architecture/reference-cutover.md#cutover-status)
for the accepted model and remaining implementation, or the [Glossary](glossary.md)
for a term. The Mac app and CLI read the same owners.

## Read by area

Read in order; each page starts from what the one before it left you with.

| You want to | Read |
|---|---|
| Install it and take one piece of work to a pull request | [Get started](getting-started.md) |
| Hand off many Tasks, steer them, and keep a longer objective moving | [Work](work.md) |
| Come back to any conversation, after a crash or from another terminal | [Sessions](sessions.md) |
| Land, repair CI, release, and run on other machines and logins | [Ops](ops.md) |
| See Work in Desktop, cmux, Herdr, tmux or your own UI | [Surfaces](surfaces.md) |
| See what agents did and what it cost | [Telemetry](telemetry.md) |
| Write Skills and Graphs and pick the agent for each step | [Orchestration](orchestration.md) |
| Fix something that went wrong | [Troubleshooting](troubleshooting.md) |
| Drive it from another AI agent | [The Agent API](agent-api.md) |
| Look up a word or a command | [Glossary](glossary.md) · [`lf` reference](lf-reference.md) |

## Shape one conversation

A skill is a markdown file that tells the coding agent what to do:

```markdown
# .lf/skills/audit.md

Audit auth changes on this branch.
Check for missing validation, confusing errors, gaps in tests.
Fix any issues you find.
```

```bash
lf audit                               # run the skill
lf audit: focus on auth                # pass arguments
lf : "fix the typo in README"          # or skip the file entirely
```

A graph chains skills with commits between them:

```yaml
# .lf/graphs/ship-api.yaml
- implement
- compress
- gate
```

```bash
lf ship-api
```

Built-ins cover the common ground: `debug`, `design`, `implement`, `compress`,
`gate`, `qa`, the `code` graph, and more. Repo skills in `.lf/skills/` override
and extend them. [Orchestration](orchestration.md) starts with a working skill and then
adds graph and goal structure only where it earns its place.

## Context

The provider loads `AGENTS.md` natively; Loopflow does not inject it again.
Every skill also sees `LOOPFLOW.md`, `scratch/`, and `.lf/wave/`.
Pull in more context explicitly:

```bash
lf gate --docs VISUAL_DESIGN.md        # one doc
lf gate --docs 'docs/*.md'             # a glob
lf gate --diff files                   # bodies of files changed on the branch
lf debug -c                            # the clipboard
lf token-compress --docs RELEASE_NOTES.md: fit this history into 2,000 tokens
```

Compression preserves decisions and evidence from the whole source.
Do not take the first N commits and call that the history.

## Where files live

```
.lf/                      # Repo config and extensions
  config.yaml             # Model, context defaults
  skills/                 # Skill prompts
  graphs/                  # Graph definitions
  wave/                   # Wave goals and memory (persists)
scratch/                  # working notes; default delivery clears added files
~/.lf/                    # Global config, skills, and the local store
  loopflow.db             # Local identity and execution history
```

`scratch/` dies with the PR; `.lf/wave/` lives forever. Design docs go in
`scratch/`, forward-looking plans in `.lf/wave/`.

## For agents

Every documentation URL serves HTML. The reviewed Markdown source remains
available to agents: append `.md` to the URL (`/docs/work.md`) or request the
canonical URL with `Accept: text/markdown`. The curated index is
[/llms.txt](/llms.txt); the complete corpus in one file is
[/llms-full.txt](/llms-full.txt). Inside a Loopflow-launched conversation you already
carry the operating contract (`LOOPFLOW.md`) — these pages are the long form.

## Next

[Get started →](getting-started.md) · [Work →](work.md) · [The Agent API →](agent-api.md)

## Reference

[Glossary](glossary.md) · [`lf` commands](lf-reference.md) · [Configuration](config.md) · [Subscriptions](subscriptions.md) · [Security](security.md) · [Troubleshooting](troubleshooting.md)

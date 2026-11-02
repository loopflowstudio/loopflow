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
lf task create "fix the flaky retry test" --run           # files it, starts its workflow, opens its conversation
lf -b task create "fix the flaky retry test" --run --graph auto   # straight to a merged PR, no stops
lf resume --task <ID>                                     # its conversation, from any terminal
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

## What else it does

You want to do something; here's how through `lf`.

| You want to… | |
|---|---|
| use every Claude and Codex login you pay for, routed to the one with room | `lf account` |
| run on Claude Code, Codex, OpenCode or Pi, per run or per Skill | `lf -a codex : "…"` |
| run on your own machine over SSH | `lf --machine mini -b task run <ID>` |
| rent a machine from Daytona, Modal, RunPod or AWS for one Task and give it back after | `lf task run <ID> --rent <template>` |
| land when checks pass, with CI failures repaired for you | `lf land --wait-and-fix` |
| see what agents cost, by Task, Wave or week | `lf history usage --weekly` |
| see which commands spend your Linear rate limit (after `logs: {linear_requests: true}` in `~/.lf/config.yaml`) | `lf history linear` |
| open work in Desktop, cmux, Herdr, tmux or a terminal | `lf open --task <ID>` |
| plan in Git with no tracker, or in Linear | `lf task list` |
| keep a longer objective moving with its own memory, plan and setup | `lf wave status` |
| cut a release | `lf release run <version>` |

## Needs

| You need | For |
|---|---|
| macOS or Linux, `git` | Everything |
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), [OpenCode](https://github.com/anomalyco/opencode) or [Pi](https://github.com/earendil-works/pi) 1.3+, signed in | Every run; each has its own cost |
| [`gh`](https://cli.github.com), signed in | Pull requests (GitHub is the code host today) |
| [Linear](https://linear.app) | Optional; the plan syncs through your Git remote by default |

## What it touches

- `~/.local/bin/lf` and `~/.lf/`: the binary, your config and a local SQLite store
  of Tasks, conversations and runs.
- Worktrees beside your repository, one per Task (`../<repo>.<task>`).
- `scratch/` in your repository for working notes; `.lf/` once you add your own
  Skills, Graphs or Waves.
- Your provider logins. There is no hosted service: conversations stay on your
  machine, and only the plan travels, as a Git ref or through Linear.

Remove it with `rm ~/.local/bin/lf && rm -rf ~/.lf`, then delete any worktrees it
created with `git worktree remove`.

## Docs

Each page picks up where the last one stops:
[Get started](docs/getting-started.md) → [Work](docs/work.md) →
[Sessions](docs/sessions.md) → [Ops](docs/ops.md) → [Surfaces](docs/surfaces.md) →
[Telemetry](docs/telemetry.md) → [Orchestration](docs/orchestration.md).
When something breaks: [Troubleshooting](docs/troubleshooting.md). Everything else:
[For agents](docs/agent-api.md) · [`lf` reference](docs/lf-reference.md).

Agents can read [llms.txt](https://loopflow.studio/llms.txt) or any page as
Markdown at its `.md` URL.

## Developing Loopflow

`AGENTS.md` is the style guide, `scripts/README.md` covers the test runner,
and `RELEASE_NOTES.md` carries the release history.

```bash
uv run python scripts/install.py local       # build under local-bin/
LF_HOME="$(mktemp -d)" local-bin/lf wave list --json
```

## License

[MIT](LICENSE)

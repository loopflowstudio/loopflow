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
  and `deliver`, each running on its configured agent in the Task's worktree.
  At a breakpoint, the Task conversation shows the finished work and the
  proposed next step. Reply to choose a direction, or run `lf task run <ID>`
  to take the recommendation.
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
- design                # writes a proposed plan in scratch/plan.md
- break                 # review the plan before implementation starts
- implement             # commits its work on the branch
- audit                 # runs on Codex per its frontmatter
- gate                  # publishes the PR so CI starts, then reviews while CI runs
```

```bash
lf design                       # work on a design together in this terminal
lf graph ship-api               # review the finished plan at your terminal
lf task run <ID> ship-api       # review the finished plan in the Task conversation
lf -b task run <ID> ship-api    # carry the proposed plan straight into implementation
```

Built-in Graphs (`feature`, `code`, `auto`, `research`, `optimize`) work out of
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

## What else it does

| You want to… | Command |
|---|---|
| Run Tasks in parallel, each in its own worktree and branch | `lf task create "…" --run` |
| Mix Claude Code, Codex, OpenCode, Pi, and Antigravity, or fail over on rate limits | `lf set --global agents claude,codex` |
| Stack a dependent Task on an open Task's branch | `lf sync <parent-ID>` |
| Adopt a dirty checkout or existing conversation as a Task | `lf task create --worktree . --title "…"` |
| Open a Task conversation here, or in a sibling tab | `lf open --task <ID>` |
| Merge when checks pass, repairing CI failures automatically | `lf land --wait-and-fix` |
| Track agent cost and context size by Task, Wave, or week | `lf session usage --weekly` |
| Run Tasks on your own machine over SSH | `lf --machine mini -b task run <ID>` |
| Rent a machine from Daytona, Modal, RunPod, or AWS for one Task | `lf task run <ID> --rent <template>` |
| Share plans in Git with no tracker, or two-way with Linear or Asana | `lf repo tracker` |
| Cut a release from version bump to tag and GitHub Release | `lf repo release run <version>` |
| Sign any Machine in to Codex, GitHub, or Linear, approved in your browser | `lf machine login codex mini` |

## Requirements

| Need | For |
|---|---|
| macOS or Linux, `git` | Everything |
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), [OpenCode](https://github.com/anomalyco/opencode), [Pi](https://github.com/earendil-works/pi) 1.3+, or [Antigravity](https://antigravity.google/docs/cli/reference), signed in | Running agents (uses your own subscriptions/keys) |
| [`gh`](https://cli.github.com), signed in | Pull requests (GitHub today) |
| [Linear](https://linear.app) or Asana | Optional; plans sync via a Git ref on `origin` by default |

## What it touches

- `~/.local/bin/lf` and `~/.lf/`: the binary, global config, and local SQLite
  stores (`loopflow.db` for active work, `history.db` for turns and history).
- Worktrees beside your repo, one per Task (`../<repo>.<task>`).
- `scratch/` for temporary working notes (cleared on submit/land); `.lf/` once
  you add custom Skills, Graphs, or Waves.
- Your native provider logins. Nothing is hosted: conversations stay local, and
  only planning state syncs.

Uninstall with `rm ~/.local/bin/lf && rm -rf ~/.lf`, then remove any worktrees
with `git worktree remove`.

## Docs

Start at the [Docs overview](docs/index.md), or read in order:
[Get started](docs/getting-started.md) → [Work](docs/work.md) →
[Sessions](docs/sessions.md) → [Ops](docs/ops.md) → [Surfaces](docs/surfaces.md) →
[Telemetry](docs/telemetry.md) → [Orchestration](docs/orchestration.md).
Reference: [Configuration](docs/config.md) · [The Agent API](docs/agent-api.md) ·
[Glossary](docs/glossary.md) · [`lf` reference](docs/lf-reference.md) ·
[Troubleshooting](docs/troubleshooting.md).

Questions or feedback: [Discord](https://discord.gg/j57B5vyXNJ).

Agents can read [llms.txt](https://loopflow.studio/llms.txt),
[llms-full.txt](https://loopflow.studio/llms-full.txt), or any page as raw
Markdown at its `.md` URL.

## Developing Loopflow

`AGENTS.md` is the style guide, `scripts/README.md` covers the test runner, and
`RELEASE_NOTES.md` carries the release history.

```bash
uv run python scripts/install.py local       # build under local-bin/
LF_HOME="$(mktemp -d)" local-bin/lf wave list --json
```

## License

[MIT](LICENSE)

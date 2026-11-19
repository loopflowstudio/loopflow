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

## What else it does

| You want to… | Command |
|---|---|
| Run Tasks in parallel, each in its own worktree and branch | `lf task create "…" --run` |
| Mix Claude Code, Codex, OpenCode, and Pi, or fail over on rate limits | `lf set --global agents claude,codex` |
| Stack a dependent Task on an open Task's branch | `lf sync <parent-ID>` |
| Adopt a dirty checkout or existing conversation as a Task | `lf task create --worktree . --title "…"` |
| Open a Task conversation here, or in a sibling tab | `lf open --task <ID>` |
| Merge when checks pass, repairing CI failures automatically | `lf land --wait-and-fix` |
| Track agent cost and context size by Task, Wave, or week | `lf session usage --weekly` |
| Run Tasks on your own machine over SSH | `lf --machine mini -b task run <ID>` |
| Rent a machine from Daytona, Modal, RunPod, or AWS for one Task | `lf task run <ID> --rent <template>` |
| Share plans in Git with no tracker, or two-way with Linear or Asana | `lf repo tracker` |
| Cut a release from version bump to tag and GitHub Release | `lf repo release run <version>` |
| Authenticate GitHub, Linear, Asana, and OpenCode | `lf account` |

## Requirements

| Need | For |
|---|---|
| macOS or Linux, `git` | Everything |
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), [OpenCode](https://github.com/anomalyco/opencode), or [Pi](https://github.com/earendil-works/pi) 1.3+, signed in | Running agents (uses your own subscriptions/keys) |
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

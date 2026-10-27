# Loopflow

```bash
curl -fsSL https://loopflow.studio/install.sh | sh
lf init
npx skills add loopflowstudio/loopflow --skill loopflow -g -y   # teach your coding agent to use lf
```

`lf` is how your agent orchestra conducts itself. It launches coding agents for
you, and lets your agents launch more. It organizes them around Tasks: one Task
ties together the tracker issue, the worktree, the branch and the pull request.
Then Waves split the work into areas, each with its own Graph, machine and
accounts. It works with Claude Code, Codex, OpenCode and Pi, on your existing
subscriptions, inside whatever terminal, multiplexer and tracker you already use.

**1. Launch agents.** In any git repository:

```bash
lf : "fix the flaky retry test"            # an agent, on whichever of your logins has room
lf -a codex : "fix the flaky retry test"   # the same thing on Codex
lf debug -c                                # a Skill: paste an error, it finds and fixes the cause
lf resume --all                            # after a crash or reboot, every interrupted conversation comes back
```

The last install line teaches your own coding agent to do the same, so you can
just ask it: *"Split the auth cleanup into Tasks and run them with lf."*
[What the skill teaches](https://github.com/loopflowstudio/loopflow/blob/main/skills/loopflow/README.md).

**2. Organize them around Tasks.** A Task is one outcome: its tracker issue, its
worktree and branch, its agents and its pull request, in one place. Ten requests
come back as ten PRs you review one at a time, and every agent on a Task starts
where the last one left off.

```bash
lf task create --title "fix the flaky retry test"   # prints the Task ID
lf -b task run <ID>                                   # worktree, branch, agents, PR
lf comment <ID> "keep the retry budget at 3"          # reaches the run before its next step
lf resume --task <ID>                                 # its conversation, from any terminal
lf land --wait-and-fix                                # merge when checks pass; fix them if not
```

**3. Split the work into Waves.** A Wave is an area of work with its own goal,
memory and plan, and its own assignments: the Graph its Tasks run, the machine
they run on and the logins that pay. Your repository is the Wave for anything
without one.

A Graph is how an area of work runs: a YAML list of Skills, and a Skill is a
Markdown file, both in your repository and reviewed like code. Each step can name
its own agent, and the whole Graph runs interactively, headless or on another
machine.

```markdown
<!-- .lf/skills/audit.md -->
Audit auth changes on this branch. Fix missing validation and untested paths.
```

```yaml
# .lf/graphs/ship-api.yaml
- implement   # leaves commits on the branch
- audit
- gate        # leaves passing checks
```

```bash
lf -b task run <ID> ship-api               # one Task through the Graph
lf wave workflow set code infrastructure   # every infrastructure Task: build, demo, then land
lf machine use mini infrastructure          # and run on your mini
```

Loopflow ships reasonable workflows for software engineering, and they are
templates. Copy one with `lf graph customize pursue`, change it, or write down
the way you work and share it.

## What else it does

You want to do something; here's how through `lf`.

| You want to… | |
|---|---|
| use every Claude and Codex login you pay for, routed to the one with room | `lf account` |
| run on Claude Code, Codex, OpenCode or Pi, per run or per Skill | `lf -a codex : "…"` |
| run on your own machine over SSH | `lf --machine mini -b task run <ID>` |
| rent a sandbox for one Task and give it back after | `lf task run <ID> --rent <template>` |
| land when checks pass, with CI failures repaired for you | `lf land --wait-and-fix` |
| see what agents cost, by Task, Wave or week | `lf history usage --weekly` |
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

`AGENTS.md` is the style guide, `TESTING.md` covers the suites, and
`RELEASE_NOTES.md` carries the release history.

```bash
uv run python scripts/install.py local       # build under local-bin/
LF_HOME="$(mktemp -d)" local-bin/lf wave list --json
```

## License

[MIT](LICENSE)

# Loopflow

```bash
curl -fsSL https://loopflow.studio/install.sh | sh
lf init
npx skills add loopflowstudio/loopflow --skill loopflow -g -y   # teach your coding agent to use lf
```

`lf` is how your agent orchestra conducts itself. At its core, it launches
coding agents and keeps their sessions. It represents workflows as graphs of
skills; each skill can name its own harness, and you run the whole graph
interactively, headless or on another machine. Hand it work as Tasks and each one
runs in its own worktree and comes back as a pull request. It works with Claude
Code, Codex, OpenCode and Pi, on your existing subscriptions, inside whatever
terminal, multiplexer and tracker you already use.

**1. Teach your agent.** The last install line gives your coding agent the
Loopflow skill. From then on you talk to your agent, and it uses `lf` for you:

> *"Split the auth cleanup into Tasks and run them with lf."*

Your agent creates the Tasks, runs each one in its own worktree, and tells you
when there's a pull request to review. [What the skill teaches](skills/loopflow/README.md).

**2. Work: every request becomes a Task with its own pull request.** Ten
requests come back as ten PRs you review one at a time. Every agent on a Task
starts where the last one left off: its notes, your comments, the docs it
should read. These are the commands your agent runs, and you can too:

```bash
lf task create --title "fix the flaky retry test"   # prints the Task ID
lf -b task run <ID>                                   # worktree, branch, agent, PR
lf comment <ID> "keep the retry budget at 3"          # reaches the run before its next step
lf task status <ID>                                   # where it stands
```

**3. Graphs: write down how each area of work runs.** A Skill is a Markdown
file and a Graph is a YAML list of Skills, both in your repository and reviewed
like code. A Task runs a Graph step to step, off your main thread.

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
lf audit                        # one Skill, here and now
lf -b task run <ID> ship-api    # one Task through the Graph, in its own worktree
```

Set it up once per area of work. A Wave, or your repository by default, chooses
the Graphs its Tasks run and the machine they run on, and every Task in it
follows that:

```bash
lf wave workflow set code infrastructure   # infrastructure Tasks build, demo, then land
lf machine use mini infrastructure          # and run on your mini
```

Loopflow ships reasonable workflows for software engineering, and they are
templates. Copy one with `lf graph customize pursue`, change it, or write down
the way you work and share it.

**4. Sessions: get back to any conversation.** Every conversation outlives its
terminal and its crash. With Tasks you never need its ID: name the Task.

```bash
lf resume --task <ID>           # the Task's conversation, from any terminal
lf resume --all                 # after a crash or reboot, every interrupted conversation comes back
lf -a codex resume <session>    # the same conversation, continued on Codex
```

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

[Get started](docs/getting-started.md) · [For agents](docs/agent-api.md) ·
[Troubleshooting](docs/troubleshooting.md) · [`lf` reference](docs/lf-reference.md)

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

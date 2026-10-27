# Loopflow

```bash
curl -fsSL https://loopflow.studio/install.sh | sh
lf init
npx skills add loopflowstudio/loopflow --skill loopflow -g -y   # teach your coding agent to use lf
```

`lf` is how your agent orchestra conducts itself. At its core, it launches
coding agents and keeps their sessions. It represents workflows as graphs of skills; each skill can name its own harness, and you run the whole graph interactively, headless or on another machine. Hand it work as Tasks and each one runs
in its own worktree and comes back as a pull request. It works with Claude Code, Codex,
OpenCode and Pi, on your existing subscriptions, inside whatever terminal, multiplexer and
tracker you already use. Three steps get you going; each works without the ones
after it.

**1. Launch: run an agent, a Skill or a Graph.** In any git repository, with
your coding agent signed in:

```bash
lf : "fix the flaky retry test"            # an agent, on whichever of your logins has room
lf -a codex : "fix the flaky retry test"   # the same thing on Codex
lf debug -c                                # a Skill: paste an error, it finds and fixes the cause
```

A Skill is a Markdown file and a Graph is a YAML list of steps, both in your
repository and reviewed like code. Write down how you work, then launch it by
name:

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
lf audit                 # one Skill
lf -b graph ship-api     # the Graph, headless, step to step
```

**2. Work: hand it off, and it runs without you.** A Task gets its own worktree,
branch and pull request, so ten requests come back as ten PRs you can review one
at a time. Every agent on it starts where the last one left off.

```bash
lf task create --title "fix the flaky retry test"   # prints the Task ID
lf -b task run <ID>                                   # its own worktree and branch; ends with a PR
lf comment <ID> "keep the retry budget at 3"          # reaches the run before its next step
```

Or ask your agent: *"Split this into Tasks and run them with lf."*

**3. Sessions: you never lose a conversation.** Every conversation outlives its
terminal, its crash and its harness's own resume list. You don't need to
remember which agent it was or what its ID is.

```bash
lf resume --all                 # after a crash or reboot, every interrupted conversation comes back
lf resume --task LOO-12         # a Task's conversation from any terminal, on whatever agent it ran
lf resume 0199a213-81c0-7800…   # one you started in plain codex or claude
```

## When you want more

Each of these builds on the steps above. Pick one up when you need it; the
linked page walks through it.

| When you want to… | Read |
|---|---|
| land the PR, with CI repaired for you | [Ops](docs/ops.md) |
| see your work in Desktop, cmux, Herdr or tmux, or build your own view | [Surfaces](docs/surfaces.md) |
| see what your agents did and what it cost | [Telemetry](docs/telemetry.md) |
| run Graphs on other machines and agents, and customize the built-in ones | [Orchestration](docs/orchestration.md) |
| keep a longer objective moving with its own memory and plan | [Waves](docs/work.md#waves) |

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

# Loopflow

```bash
curl -fsSL https://loopflow.studio/install.sh | sh
lf init
npx skills add loopflowstudio/loopflow --skill loopflow -g -y   # teach your coding agent to use lf
```

`lf` is the command you type *and* the CLI your coding agent uses to get work
done. Works with Claude Code, Codex, OpenCode and Pi, on your own logins.

**Work: say what you want, and it becomes Tasks that run without you.** Each
Task gets its own worktree, branch and pull request, so ten requests come back
as ten PRs you can review one at a time.

```bash
lf task create --title "rent CPU pods for templates with no GPU"   # prints the Task ID
lf -b task run <ID>
```

**Context: every agent starts where the last one left off.** Each launch carries
the Wave's memory, the Task's notes in `scratch/`, the docs you name and any new
comments, and gets them back after compaction.

```bash
lf comment <ID> "default to the smallest pod"    # reaches the run before its next step
lf gate --docs docs/runpod.md                    # name what it should read
```

**Orchestration: work finishes while you're doing something else.** Runs go to
the agent and machine you pick, Graphs carry them step to step, Waves keep their
Tasks moving, and landing repairs CI until the PR merges. Get as much as
possible off the main thread: plans into code, code into verified code.

```bash
lf --machine mini -b -a codex graph pursue   # a Graph, headless, on Codex, on your mini
lf land --wait-and-fix          # merge when checks pass; fix them if not
```

**Sessions: you never lose a conversation.** Every conversation outlives its
terminal, and new work starts on whichever of your logins has room. Close the
window and come back next week; the conversation and what it knows are still
there.

```bash
lf resume --task <ID>
```

**Skill Graphs: write down how you work.** Skills are Markdown and Graphs are
YAML, files in your repository reviewed like code.

```markdown
<!-- .lf/skills/audit.md -->
Audit auth changes on this branch. Fix missing validation and untested paths.
```

```yaml
# .lf/graphs/ship-api.yaml
- design      # leaves scratch/<task>.md
- implement   # leaves commits on the branch
- audit
- gate        # leaves passing checks
```

```bash
lf audit                 # run one Skill
lf -b graph ship-api     # run the Graph headless
```

Loopflow ships reasonable workflows for software engineering, and they are
templates. Copy one with `lf graph customize pursue`, change it, or write down
the way you work and share it.

## Try it on one small thing

Use what you like and skip what you don't: the launcher needs no Task and no
Linear. In any git repository, with your coding agent signed in:

```bash
lf : "fix the flaky retry test"        # one agent, like starting it yourself
```

Then hand the same thing off as a Task:

```bash
lf task create --title "fix the flaky retry test"   # prints the Task ID
lf -b task run <ID>                                   # its own worktree and branch; ends with a PR
lf resume --task <ID>                                 # its conversation, whenever you want it
```

Or ask your agent: *"Use lf to split this into Tasks and run them."*

## Needs

| You need | For |
|---|---|
| macOS or Linux, `git` | Everything |
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), [OpenCode](https://github.com/anomalyco/opencode) or [Pi](https://github.com/earendil-works/pi) 1.3+, signed in | Every run; each has its own cost |
| [`gh`](https://cli.github.com), signed in | Pull requests |
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
[Skills and Graphs](docs/authoring.md) · [Tasks and planning](docs/planning.md) ·
[Waves](docs/waves.md) · [`lf` reference](docs/lf.md) ·
[Troubleshooting](docs/troubleshooting.md)

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

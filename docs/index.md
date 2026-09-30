# Loopflow

A software instrument. It doesn't make the software for you. You make the
software through it.

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
lf init
```

AI can build a lot of software fast. It can also spend all day going in
circles, and it's hard to tell which is happening. Loopflow keeps track, so
you can see what got done and what still needs you.

Free and open source. Needs [Claude Code](https://docs.anthropic.com/en/docs/claude-code)
or [Codex](https://github.com/openai/codex), which have their own cost.

## The pieces

Loopflow is built from a few pieces. Each one is small, and most are plain
text files you can open and change.

| Piece | What it is | Where it lives |
|---|---|---|
| **Skill** | One step: instructions for the AI, written as a text file | `.lf/skills/*.md` |
| **Flow** | Steps in order. One that can go back and try again is a loopflow | `.lf/flows/*.yaml` |
| **Task** | One piece of work with a finish line, done in its own copy of your project | Linear, Git, and GitHub |
| **Wave** | A goal Loopflow keeps working on, with its own memory and schedule | `wave/<name>/` |
| **Project** | The current plan for a Wave, with results you can check | Linear |
| **Run** | A record of one time the AI coding tool was started | `$LF_HOME/runs/` on the computer that ran it |

Each piece keeps one kind of fact, so you always know where to look. New
terms are in the [Glossary](glossary.md).

The Mac app shows all of this in one window: your goals, their tasks, and
the conversations that need you. It reads the same information the commands
below print, so anything you see in the app you can also ask for in a
terminal.

```bash
lf --wave engbot wave/operate "ship the parser fix first"
lf wave status engbot                                  # where it stands
```

## Read by area

| You want to | Read |
|---|---|
| Install it and try one piece of work | [Get Started](getting-started.md) |
| See what got done and what needs you | [Conducting](conducting.md) |
| Give it a goal to keep working on | [Waves](waves.md) |
| Change how it works | [Authoring](authoring.md) |
| Drive it from another AI agent | [The Agent API](agent-api.md) |
| Look up a word | [Glossary](glossary.md) |
| Look up a command | [`lf` reference](lf.md) |

## Shape one run

A skill is a markdown file that tells the coding agent what to do:

```markdown
# .lf/skills/audit.md

Audit auth changes on this branch.
Check for missing validation, confusing errors, gaps in tests.
Fix any issues you find.
```

```bash
lf audit                      # run the skill
lf audit: focus on auth       # pass arguments
lf : "fix the typo in README" # or skip the file entirely
```

A flow chains skills with commits between them:

```yaml
# .lf/flows/ship-api.yaml
- implement
- compress
- gate
```

```bash
lf ship-api
```

Built-ins cover the common ground: `debug`, `design`, `implement`, `compress`,
`gate`, `qa`, the `code` flow, and more. Repo skills in `.lf/skills/` override
and extend them. [Authoring](authoring.md) starts with a working skill and then
adds flow and goal structure only where it earns its place.

## Context

Every skill sees your agent doc (`AGENTS.md` / `CLAUDE.md`), `LOOPFLOW.md`,
`scratch/`, and `wave/`. Nothing else is auto-injected — pull in more
explicitly:

```bash
lf gate --docs VISUAL_DESIGN.md      # one doc
lf gate --docs 'docs/*.md'           # a glob
lf gate --diff-files                 # bodies of files changed on the branch
lf debug -c                          # the clipboard
lf token-compress --docs RELEASE_NOTES.md: fit this history into 2,000 tokens
```

Compression preserves decisions and evidence from the whole source.
Do not take the first N commits and call that the history.

## Where files live

```
.lf/                      # Repo config and extensions
  config.yaml             # Model, context defaults
  skills/                 # Skill prompts
  flows/                  # Flow definitions
scratch/                  # PR scratchpad (cleared on merge)
wave/                     # Wave goals and memory (persists)
~/.lf/                    # Global config, skills, and the local store
  runs/                   # Home-local append-only launch evidence
```

`scratch/` dies with the PR; `wave/` lives forever. Design docs go in
`scratch/`, forward-looking plans in `wave/`.

## For agents

Every documentation URL serves HTML. The reviewed Markdown source remains
available to agents: append `.md` to the URL (`/docs/waves.md`) or request the
canonical URL with `Accept: text/markdown`. The curated index is
[/llms.txt](/llms.txt); the complete corpus in one file is
[/llms-full.txt](/llms-full.txt). Inside a Loopflow-launched Run you already
carry the operating contract (`LOOPFLOW.md`) — these pages are the long form.

## Next

[Get Started →](getting-started.md) · [Waves →](waves.md) · [The Agent API →](agent-api.md) · [Conducting →](conducting.md)

## Reference

[Glossary](glossary.md) · [`lf` commands](lf.md) · [Authoring](authoring.md) · [Configuration](config.md) · [Subscriptions](subscriptions.md) · [Security](security.md) · [Troubleshooting](troubleshooting.md)

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
| **Project** | The internal record of a Wave’s current chapter plan, with results to check | Linear |
| **Run** | A record of one time the AI coding tool was started | `$LF_HOME/runs/` on the computer that ran it |

A [repository](glossary.md#borrowed-from-software-engineering) is a project
folder tracked by Git. Git saves changes as commits, and a branch keeps one
line of work separate until it can be merged into the shared version. Linear
stores the Tasks; GitHub hosts their pull requests for review before merge.
The [Glossary](glossary.md) defines these and the other terms used below.

The Mac app shows all of this in one window: your goals, their tasks, and
the conversations that need you. It reads the same information the commands
below print, so anything you see in the app you can also ask for in a
terminal.

```bash
lf start engbot                                   # start a Wave you wrote at wave/engbot/GOAL.md
lf --wave engbot wave/operate "ship the parser fix first"
lf status engbot                                  # where it stands
lf stop engbot
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

Audit sign-in changes on this branch.
Check for missing validation, confusing errors, gaps in tests.
Fix any issues you find.
```

```bash
lf audit                      # run the skill
lf audit: focus on sign-in    # add instructions after the colon
lf : "fix the typo in README" # or skip the file entirely
```

A Flow chains skills with commits between them, saving a checkpoint after each step:

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
`gate`, `qa`, the `build` flow, and more. Repo skills in `.lf/skills/` override
and extend them. [Authoring](authoring.md) starts with a working skill and then
adds flow and goal structure only where it earns its place.

## Context

[Context](glossary.md#loopflows-words) is the information given to the AI with
its instructions. Every skill sees your agent guide (`AGENTS.md` / `CLAUDE.md`),
`LOOPFLOW.md`, `scratch/`, and `wave/`. Extra files are opt-in, leaving input
space for the task. Add what the work needs:

```bash
lf gate --docs VISUAL_DESIGN.md      # one doc
lf gate --docs 'docs/*.md'           # a glob: every matching Markdown file
lf gate --diff-files                 # bodies of files changed on the branch
lf debug -c                          # the clipboard
lf token-compress --docs RELEASE_NOTES.md: fit this history into 2,000 tokens
```

Tokens are the text units counted by the AI. When shortening context to fit
a token budget, preserve decisions and evidence across the whole source;
taking only its first few entries loses later changes.

## Where files live

```
.lf/                      # Repo config and extensions
  config.yaml             # Model, context defaults
  skills/                 # Skill prompts
  flows/                  # Flow definitions
scratch/                  # PR scratchpad (cleared on merge)
wave/                     # Wave goals and memory (persists)
~/.lf/                    # Global config, skills, and the local store
  runs/                   # launch records on this computer; new entries preserve history
```

Delivery preparation clears `scratch/`. Keep designs and temporary feedback
there; keep the Wave’s continuing objective and lessons under `wave/`. Its
current chapter’s Tasks and targets live in Linear.

## For agents

Every documentation URL serves HTML, the format rendered as a web page. The reviewed Markdown source remains
available to agents: append `.md` to the URL (`/docs/waves.md`) or request the
canonical URL with `Accept: text/markdown`. The curated index is
[/llms.txt](/llms.txt); the complete corpus in one file is
[/llms-full.txt](/llms-full.txt). Inside a Loopflow-launched Run you already
carry the operating contract (`LOOPFLOW.md`) — these pages are the long form.

## Next

[Get Started →](getting-started.md) · [Waves →](waves.md) · [The Agent API →](agent-api.md) · [Conducting →](conducting.md)

## Reference

[Glossary](glossary.md) · [`lf` commands](lf.md) · [Authoring](authoring.md) · [Configuration](config.md) · [Subscriptions](subscriptions.md) · [Security](security.md) · [Troubleshooting](troubleshooting.md)

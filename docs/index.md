# Loopflow

Run one useful prompt first:

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
lf init
lf debug -c        # copy an error to the clipboard, watch it fix
```

That command assembles the skill, repository guidance, scratch notes, and
clipboard into one prompt. It launches the configured provider and writes one
Home-local Run record. The Run records what happened; it does not reserve the
repository, control a Wave, or become planning state.

The same building block runs **Waves**: persistent agents that coordinate
Linear-backed Projects and Tasks, remember what they learn, and stay steerable.
There is no Loopflow server at the center. Repo files hold authored behavior;
Linear and GitHub hold shared coordination and delivery facts; each Home keeps
its local execution records. `lf ssh` runs the same local commands on another
Home.

## Build outward

| Add this | When you need it | Truth lives in |
|---|---|---|
| **Skill** | One repeatable agent action | `.lf/skills/*.md` |
| **Flow** | Several bounded actions in order | `.lf/flows/*.yaml` |
| **Wave** | A durable goal, memory, cadence, and project selection | `wave/<name>/` plus local placement |
| **Project** | A measured bet with KRs | Linear |
| **Task** | One concrete change in its own worktree | Linear, Git, and GitHub |
| **Run** | Evidence from one harness launch | `$LF_HOME/runs/` on the executing Home |

Each layer owns one kind of fact. None needs a universal current execution
record to coordinate the others.

Start a Wave after authoring `wave/engbot/GOAL.md`:

```bash
lf start engbot
lf chat --steer "ship the parser fix first"
lf status engbot
lf stop engbot
```

**Loopflow** (macOS) presents the same conversation and work map. It is a
client of the same local `lf --json` reads, not another source of truth.

## Read by area

| Where you are | What to read |
|---|---|
| Just installed, want to try it | [Get Started](getting-started.md) |
| Automating with a persistent agent | [Waves](waves.md) |
| Writing skills, flows, and goals | [Authoring](authoring.md) |
| Writing an agent that drives other agents | [The Agent API](agent-api.md) |
| Watching and steering many agents | [Conducting](conducting.md) |
| Looking up a command | [`lf` reference](lf.md) |

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

A direction shapes how the agent judges:

```bash
lf gate --direction ux            # optimize for user experience
lf gate --direction ux,clarity    # stack intents
```

Built-ins cover the common ground: `debug`, `design`, `implement`, `compress`,
`gate`, `qa`, the `build` flow, and more. Repo skills in `.lf/skills/` override
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
  directions/             # Judgment and intent
  flows/                  # Flow definitions
scratch/                  # PR scratchpad (cleared on merge)
wave/                     # Wave goals and memory (persists)
~/.lf/                    # Global config, skills, and the local store
  runs/                   # Home-local append-only launch evidence
```

`scratch/` dies with the PR; `wave/` lives forever. Design docs go in
`scratch/`, forward-looking plans in `wave/`.

## For agents

Every human docs URL serves HTML. The reviewed Markdown source remains
available to agents: append `.md` to the URL (`/docs/waves.md`) or request the
canonical URL with `Accept: text/markdown`. The curated index is
[/llms.txt](/llms.txt); the complete corpus in one file is
[/llms-full.txt](/llms-full.txt). Inside a Loopflow-launched Run you already
carry the operating contract (`LOOPFLOW.md`) — these pages are the long form.

## Next

[Get Started →](getting-started.md) · [Waves →](waves.md) · [The Agent API →](agent-api.md) · [Conducting →](conducting.md)

## Reference

[`lf` commands](lf.md) · [Authoring](authoring.md) · [Configuration](config.md) · [Subscriptions](subscriptions.md) · [Security](security.md) · [Troubleshooting](troubleshooting.md)

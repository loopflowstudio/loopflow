# Loopflow

```bash
curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh
lf init
lf debug -c
```

A software instrument. Give an agent a task, keep its conversation, and resume
captured work when a command stops. Loopflow keeps the command's result, the
agent's history and the Flow's progress separate, so each answers one question.

Free and open source. Needs [Claude Code](https://docs.anthropic.com/en/docs/claude-code)
or [Codex](https://github.com/openai/codex), which have their own cost.

## The pieces

```bash
lf -b implement
lf session list --interactive false --json
lf session connect SESSION
lf --task INF-123 flow start
```

A Skill supplies instructions; a Flow composes skills, mechanical operations and
reviews. A Task owns one change, its checkout and PRs. A Wave keeps the objective
and memory; its current Linear Project holds the plan and default Flow.

Exec records an actual lf command process. AgentSession keeps the conversation,
including headless work. FlowSession preserves a captured Flow and consumes its
exact boundary results. A conversation can outlive its command, and a completed
command can leave a Flow waiting for review.

Read the [contract and cutover status](architecture-reference.md#cutover-status)
for the accepted model and remaining implementation, or the [Glossary](glossary.md)
for a term. The Mac app and CLI read the same owners.

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

## Shape one conversation

A skill is a markdown file that tells the coding agent what to do:

```markdown
# .lf/skills/audit.md

Audit auth changes on this branch.
Check for missing validation, confusing errors, gaps in tests.
Fix any issues you find.
```

```bash
lf audit                               # run the skill
lf audit: focus on auth                # pass arguments
lf : "fix the typo in README"          # or skip the file entirely
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
lf gate --docs VISUAL_DESIGN.md        # one doc
lf gate --docs 'docs/*.md'             # a glob
lf gate --diff files                   # bodies of files changed on the branch
lf debug -c                            # the clipboard
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
  loopflow.db             # Local identity and execution history
```

`scratch/` dies with the PR; `wave/` lives forever. Design docs go in
`scratch/`, forward-looking plans in `wave/`.

## For agents

Every documentation URL serves HTML. The reviewed Markdown source remains
available to agents: append `.md` to the URL (`/docs/waves.md`) or request the
canonical URL with `Accept: text/markdown`. The curated index is
[/llms.txt](/llms.txt); the complete corpus in one file is
[/llms-full.txt](/llms-full.txt). Inside a Loopflow-launched conversation you already
carry the operating contract (`LOOPFLOW.md`) — these pages are the long form.

## Next

[Get Started →](getting-started.md) · [Waves →](waves.md) · [The Agent API →](agent-api.md) · [Conducting →](conducting.md)

## Reference

[Glossary](glossary.md) · [`lf` commands](lf.md) · [Authoring](authoring.md) · [Configuration](config.md) · [Subscriptions](subscriptions.md) · [Security](security.md) · [Troubleshooting](troubleshooting.md)

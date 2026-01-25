# Loopflow

Arrange and conduct an agent orchestra. 

Loopflow helps you maintain flow and craft using Claude Code or other coding agents at high scale.

## Agents

The core entities in loopflow are agents. You work together with agents, which are configured and prompted coding agents, to produce software.

Agents are made of 4 fields. The art of loopflow is remixing across these 4 dimensions in novel ways.

| Field | Usage |
|------|--------------|
| **Area** | Scope and context |
| **Flow** | Process followed / steps taken |
| **Goal** | Defines success, quality, and aesthetics |
| **Stimulus** | Watch, loop, or cron |

## Steps

But before you make an agent, let's start at the beginning. All agents take their first steps.

Steps are the building blocks of flows and can also be used for small, atomic changes.

There are many built-in steps that come bundled with loopflow, such as implement, polish, and rebase, but you can also write your own (put markdown in `.claude/commands` or `.lf/steps`) or import from your favorite library.

For example, try:

```bash
lf debug -c
```

This runs the `debug` prompt, loading the clipboard (`-c`) as context on what to debug. Steps are, by default, non-interactive. If you've copied a failing test to your clipboard, your coding agent will now go fix it for you.

To start building something new try:

```bash
lf design
```

This will trigger an interactive session with a coding agent primed to design something new in your codebase with you.

## Flows

You chain steps together to make flows.  Start by trying it manually:

```bash
lf design                 # create a design doc (interactive)
lf implement && lf reduce && lf polish && lfops pr # turn it into shippable code!
```

You can also pre-register flows. `ship` is a built-in flow that names the `implement -> reduce -> polish` flow.

```bash
lf flow ship
```

## Running Agents

Once you have played with chaining steps into flows, you're ready to start playing with agents.

```bash
lfd create engbot --area src/ --goal product-engineer --flow ship
```

Runs the `ship` flow on `src/` continuously using the `product-engineer` prompt to guide direction, creating PRs until stopped.

```bash
lfd loop engbot      # keep shipping continuously
lfd subscribe ship designs/ -g designer   # or only ship when new designs arrive
```

You can compose multiple goals to add additional nuance or perspectives.

```bash
lf review -g designer,product-engineer
lf review -g ceo    
```

## Install

```bash
uv tool install loopflow
```

Built-in steps and flows included. `lf init` sets up Claude Code and preferences.

[Documentation →](docs/index.md)

## Integrations

**Coding Agents**
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — Anthropic's coding agent (default)
- [Codex CLI](https://github.com/openai/codex) — OpenAI's coding agent
- [Gemini CLI](https://github.com/google-gemini/gemini-cli) — Google's coding agent

**Tools**
- [worktrunk](https://github.com/loopflowstudio/worktrunk) — git worktree management (`wt` commands)

**Skill Libraries**
- [superpowers](https://github.com/obra/superpowers) — prompt library (`lf sp:<skill>`)
- [SkillRegistry](https://skillregistry.io/) — remote skill directory (`lf sr:<skill>`)
- [rams](https://rams.ai) — accessibility and visual design review

## Requirements

- macOS
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), or [Gemini CLI](https://github.com/google-gemini/gemini-cli)

## License

MIT

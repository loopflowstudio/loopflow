# Loopflow

Loopflow helps you maintain flow and craft using coding agents (Claude Code, Codex, Gemini CLI) at high scale.

## Waves

The core entities in loopflow are waves. Waves are a new way of configuring and using coding agents in composable and autonmous ways.

Waves are made of 4 fields.

| Field | Usage |
|------|--------------|
| **Area** | Scope and context |
| **Flow** | Process followed / steps taken |
| **Direction** | Defines success, quality, and aesthetics |
| **Stimulus** | Watch, loop, or cron |

## Steps

But before you create a wave, let's start at the beginning. 

**Steps** are the the most basic of Loopflow building blocks. Steps are simply prompts for running coding agents to execute concrete, scoped, atomic tasks.

There are many built-in steps that come bundled with loopflow, such as implement, polish, and rebase, but you can also write your own (put markdown in `.claude/commands` or `.lf/steps`) or import from your favorite library.

For example, try:

```bash
lf debug -c
```

This runs the `debug` step, loading the clipboard (`-c`) as context on what to debug. Steps are non-interactive by default. If you've copied a failing test to your clipboard, the coding agent will fix it for you.

To start building something new try:

```bash
lf design
```

This starts an interactive session with a coding agent, primed to design something new in your codebase with you.

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

## Playing in the Waves

Once you have played with chaining steps into flows, you're ready to start running waves.

```bash
lfd create engbot --area src/ --direction product-engineer --flow ship
```

Runs the `ship` flow on `src/` continuously using the `product-engineer` direction, creating PRs until stopped.

```bash
lfd loop engbot      # keep shipping continuously
lfd subscribe ship designs/ -d designer   # or only ship when new designs arrive
```

You can compose multiple directions to add additional nuance or perspectives.

```bash
lf review -d designer,product-engineer
lf review -d ceo
```

## Install

```bash
uv tool install loopflow
```

Built-in steps and flows included. `lf init` sets up your coding agent and preferences.

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

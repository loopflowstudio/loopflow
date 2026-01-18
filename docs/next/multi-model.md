---
layout: default
title: Multi-Model
---

# Multi-Model Execution

*Coming soon.*

Run tasks with multiple models in parallel or race them against each other.

## Model Racing

Race models against each other, pick the winner:

```bash
lf implement --race claude,codex: add caching layer
```

This runs both models in parallel worktrees, then uses a judge prompt to pick the better implementation.

## Parallel Execution

For parallel without judging:

```bash
lf implement --parallel claude,codex: add caching
```

Compare results manually with `lfwt compare`.

## Model Selection

Override the default model per-run:

```bash
lf review -m codex              # use Codex
lf implement -m claude:opus     # use Claude Opus
lf debug -m gemini              # use Gemini
```

## Configuration

Set the default model in `.lf/config.yaml`:

```yaml
agent_model: claude:opus
```

Available backends:
- `claude` — Claude Code (default: opus)
- `codex` — Codex CLI (default: o3)
- `gemini` — Gemini CLI (default: 2.5-pro)

Use `backend:variant` to specify a model variant.

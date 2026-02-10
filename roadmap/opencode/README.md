# OpenCode Agent Integration

Add [OpenCode](https://github.com/anomalyco/opencode) as a fourth coding agent alongside Claude, Codex, and Gemini.

## North Star

`lf implement --agent opencode` works end-to-end: prompt assembly, headless execution, stream parsing, model selection. OpenCode becomes a first-class citizen with the same fidelity as the existing three agents.

## Context

OpenCode (anomalyco/opencode) is a TypeScript-based CLI coding agent with:
- Non-interactive mode: `opencode run "prompt"`
- Model selection: `--model provider/model` (e.g., `anthropic/claude-sonnet-4-5`)
- JSON streaming: `--format json` produces NDJSON (one JSON object per line)
- Permission auto-approve: `"permission": "allow"` in config
- Context injection: `AGENTS.md`, `instructions` array in `opencode.json`, `{file:...}` references
- API keys: standard env vars (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, etc.)
- Install: `brew install anomalyco/tap/opencode` or `npm i -g opencode`

**Note:** There is a separate, unrelated project at opencode-ai/opencode (Go-based, uses `-p` flag). We target anomalyco/opencode (opencode.ai).

## Design Priorities

- **Respect OpenCode's CLI**: pass through model strings, use `opencode run`
- **Minimal config surface**: opencode's own `opencode.json` handles provider setup
- **Same fidelity as existing agents**: streaming, model variants, context injection
- **No default model variant**: like codex, let opencode use its own config

## Status

Complete. Model string format and permission handling documented in `docs/config.md`.

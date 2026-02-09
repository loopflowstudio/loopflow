# OpenCode Agent Integration — Design Notes

## What to build

OpenCode (anomalyco/opencode) as a fourth coding agent in loopflow, with the same integration fidelity as Claude, Codex, and Gemini.

## Key Finding: Two OpenCodes

There are two unrelated projects called "opencode":
- **opencode-ai/opencode** — Go-based, `-p` flag, smaller project
- **anomalyco/opencode** (opencode.ai) — TypeScript, `opencode run`, 95k+ stars, actively maintained

We target **anomalyco/opencode**.

## OpenCode CLI Summary

| Capability | Syntax |
|-----------|--------|
| Non-interactive | `opencode run "prompt"` |
| Model selection | `--model provider/model` |
| JSON streaming | `--format json` → NDJSON |
| Permission auto-approve | `OPENCODE_CONFIG_CONTENT='{"permission":"allow"}'` |
| Context injection | `OPENCODE_CONFIG_CONTENT='{"instructions":["path"]}'` |
| Interactive | `opencode` (launches TUI) |

## NDJSON Streaming Format

Line-delimited JSON. Each line has `type`, `timestamp`, `sessionID`, `part`:

```json
{"type":"step_start","timestamp":...,"sessionID":"ses_...","part":{"type":"step-start"}}
{"type":"text","timestamp":...,"sessionID":"ses_...","part":{"type":"text","text":"Hello..."}}
{"type":"step_finish","timestamp":...,"sessionID":"ses_...","part":{"type":"step-finish","tokens":{...},"cost":0.05}}
```

No conflicts with existing agent event types. `"text"` is guarded by checking for `sessionID` field.

## Context Injection Strategy

Use `OPENCODE_CONFIG_CONTENT` env var — merges JSON config at runtime without touching user's `opencode.json`:

```json
{"permission":"allow","instructions":["/tmp/lf-context-abc123.md"]}
```

OpenCode also reads `AGENTS.md` and `CLAUDE.md` from project root automatically.

## PR Sequencing

| PR | Files | ~Lines | Description |
|----|-------|--------|-------------|
| 01 | `agent.rs`, `mod.rs` | ~150 | `build_opencode_command()`, dispatch, permission env var, unit tests |
| 02 | `stream.rs` | ~200 | NDJSON parser: text, step_start, step_finish events + tests |
| 03 | `agent.rs` | ~100 | Unified `OPENCODE_CONFIG_CONTENT` builder with permission + instructions |
| 04 | `agent.rs`, `stream.rs`, `README.md` | ~150 | Remaining test coverage, docs |

## Open Questions

- Tool call events in NDJSON — format unknown, handle via `Passthrough` for now
- `opencode run` may hang if `OPENCODE_CONFIG_CONTENT` permission override doesn't work — needs live testing
- Interactive mode: opencode TUI inherits stdio, should work with `launch_interactive` as-is

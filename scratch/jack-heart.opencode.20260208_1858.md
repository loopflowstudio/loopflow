# OpenCode Agent Integration

## Status

PR 01 (command builder + launch) is complete. PRs 02-04 remain.

| PR | Status | Scope |
|----|--------|-------|
| 01 | **Done** | `build_opencode_command`, dispatch, `OPENCODE_CONFIG_CONTENT` env var, 4 unit tests |
| 02 | Next | NDJSON stream parser (`text`, `step_start`, `step_finish`) |
| 03 | Todo | Context injection wiring (callers pass `context_file` for opencode) |
| 04 | Todo | Test coverage gaps, README/docs updates |

## Key decisions (PR 01)

- **Unified env var builder from the start.** `OPENCODE_CONFIG_CONTENT` block builds a `serde_json::Map` handling both `permission` and `instructions`. No refactor needed in PR 03.
- **Context injection via env var, not CLI flag.** Follows the Gemini pattern (`GEMINI_SYSTEM_MD`), not Claude's (`--append-system-prompt-file`).
- **No default model variant.** `parse_model("opencode")` returns `None` — same as Codex. OpenCode manages its own model config.

## OpenCode CLI reference

| Capability | Syntax |
|-----------|--------|
| Non-interactive | `opencode run "prompt"` |
| Model selection | `--model provider/model` |
| JSON streaming | `--format json` → NDJSON |
| Permission auto-approve | `OPENCODE_CONFIG_CONTENT='{"permission":"allow"}'` |
| Context injection | `OPENCODE_CONFIG_CONTENT='{"instructions":["path"]}'` |
| Interactive | `opencode` (launches TUI) |

## NDJSON format (for PR 02)

```json
{"type":"step_start","timestamp":...,"sessionID":"ses_...","part":{"type":"step-start"}}
{"type":"text","timestamp":...,"sessionID":"ses_...","part":{"type":"text","text":"Hello..."}}
{"type":"step_finish","timestamp":...,"sessionID":"ses_...","part":{"type":"step-finish","tokens":{...},"cost":0.05}}
```

Disambiguate from other agents via `sessionID` field. No type conflicts with Claude/Codex/Gemini.

## Open questions

- Tool call events in NDJSON — format unknown, handle via `Passthrough` for now
- `OPENCODE_CONFIG_CONTENT` behavior based on docs, not live testing — if the env var doesn't merge correctly, auto mode could hang on permission prompts
- `opencode run` exit behavior on permission denial unknown (non-zero exit vs hang)

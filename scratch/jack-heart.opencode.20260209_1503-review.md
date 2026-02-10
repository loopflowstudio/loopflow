# OpenCode Integration — Review

## What was implemented

Full integration of OpenCode as a fourth coding agent alongside Claude, Codex, and Gemini. Three areas of work:

1. **Command builder + launch** (`agent.rs`): `build_opencode_command` constructs `opencode run` with `--model` and `--format json` flags. `build_opencode_env` constructs `OPENCODE_CONFIG_CONTENT` JSON for permission auto-approve and context file injection. `launch_agent` sets the env var when spawning opencode.

2. **Stream parser** (`stream.rs`): Three match arms in `feed_line` handle `text`, `step_start`, and `step_finish` NDJSON events. Two helper functions (`parse_opencode_text`, `parse_opencode_finish`) extract text content and cost from OpenCode's `part`-wrapped JSON format.

3. **Tests and docs**: 18 opencode-specific tests across `agent.rs`, `stream.rs`, and `config.rs`. README and `docs/config.md` updated to list opencode as a supported backend.

## Key choices

- **`sessionID` guard on `"text"` only.** `step_start` and `step_finish` are unambiguous — no other agent uses them. `"text"` gets a guard because it's generic enough for future conflicts.
- **Stateless parser.** No struct changes. Cost comes from `step_finish.part.cost`; duration is `None` because computing it from timestamp pairs would require state.
- **Unknown types → Passthrough.** Tool events (`tool_call`, etc.) fall through the catch-all `_` arm and print raw JSON. Structured parsing comes when we capture real samples.
- **Single env var for all config.** `OPENCODE_CONFIG_CONTENT` carries both `permission` and `instructions` keys. No user config files modified.
- **No default model variant.** Like codex, `parse_model("opencode")` returns `None` for variant — opencode uses its own config to pick the default model.
- **Extracted `build_opencode_env` for testability.** Pure function: config in, `Option<String>` out. Four unit tests cover all combinations.

## How it fits together

`build_model_command` dispatches to `build_opencode_command` when backend is `"opencode"`. The command builder produces `["opencode", "run", "--model", variant, "--format", "json"]` for auto+streaming mode, or just `["opencode", "run"]` for interactive.

`launch_agent` calls `build_opencode_env` to construct the env var JSON, then sets `OPENCODE_CONFIG_CONTENT` on the child process. This injects both permission auto-approve and the context temp file path.

`feed_line` dispatches OpenCode events between the Gemini section and the shared `"result"` arm. The helpers follow the same pattern as other agents — extract from JSON, return `Option<StreamEvent>` or `StreamEvent`.

## Risks

- **Tool events are unstructured.** If OpenCode emits `tool_call` events during real usage, they'll show as raw JSON via `Passthrough`. Intentional degradation — no data loss, just less formatting.
- **`step_start`/`step_finish` naming.** If a future agent uses these same type strings, they'd be caught by these arms. Low risk — the `sessionID` guard pattern is available if needed.
- **No integration test with real opencode.** All tests use fixture JSON and mock configs. Verified that the command and env var construction is correct, but haven't tested against a running opencode process.

## Not included

- Tool event parsing (no real samples to work from)
- Duration computation (would require stateful parser)
- Token count extraction (no `StreamEvent` field for it)
- Golden prompt test for opencode (would need opencode-specific golden file)

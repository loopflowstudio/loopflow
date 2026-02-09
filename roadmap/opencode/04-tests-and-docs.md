---
status: todo
phase: 4
---
# OpenCode Tests and Docs

Full test coverage for all opencode integration code, plus README and config doc updates.

## Current

Each agent has:
- Unit tests in `agent.rs` (command building)
- Stream parser tests in `stream.rs`
- Golden prompt tests (`cargo test -p loopflow golden_prompt`)
- Parity tests (`uv run pytest tests/parity/test_prompt_parity.py`)
- Listed in README.md integrations section

## Build

### Consolidate tests from PRs 01-03

PRs 01-03 each include inline tests. This PR adds any missing coverage:

- `parse_model("opencode")` → `("opencode", None)` — no default variant
- `parse_model("opencode:openai/gpt-4o")` → `("opencode", Some("openai/gpt-4o"))`
- `build_agent_command("opencode", "fix the bug", &config)` → full command with prompt
- `build_agent_command("opencode:anthropic/claude-sonnet", ...)` → model parsed and passed
- `OPENCODE_CONFIG_CONTENT` env var correctness with both permission + instructions
- Stream parser: empty text events → `None`
- Stream parser: malformed JSON → `Passthrough`

### Documentation updates

**README.md** — add opencode to integrations:

```markdown
**Coding Agents**
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — Anthropic's coding agent (default)
- [Codex CLI](https://github.com/openai/codex) — OpenAI's coding agent
- [Gemini CLI](https://github.com/google-gemini/gemini-cli) — Google's coding agent
- [OpenCode](https://github.com/anomalyco/opencode) — Open source coding agent
```

**Config docs** — document opencode model format:

```yaml
agent_model: opencode                          # use opencode's default model
agent_model: opencode:anthropic/claude-sonnet  # explicit model
agent_model: opencode:openai/gpt-4o            # any provider opencode supports
```

## Constraints

- Tests must not require opencode to be installed (test command building and parsing, not execution)
- Stream parser tests use fixture JSON strings from the NDJSON format documented in PR 02
- No default model variant for opencode — verify this explicitly

## Done when

```bash
cargo test -p loopflow -- opencode
cargo fmt --check
cargo clippy -- -D warnings
```

All pass with zero warnings. README lists opencode as a supported agent.

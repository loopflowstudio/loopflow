# OpenCode conformance traces and schema pinning

## Problem

OpenCode is the only harness without replay tests from real provider output. Today we rely on inferred field fallbacks (`sessionID`/`sessionId`/`session_id`, etc.), which can silently map the wrong payload and hide breakage until users hit it in live sessions.

This directly blocks two `wave/agentapi` success criteria:

- Goal: **"Harness-agnostic: same client code works regardless of which agent runs the session"**
- Metric: **"All three harnesses (Codex, Claude, OpenCode) pass shared conformance tests"**

## Approach

Capture real OpenCode wire data, replay it in the existing conformance harness, then pin mapping code to the observed schema.

1. **Add a trace capture script** (`scripts/record_opencode_conformance_trace.py`).
   - Starts `opencode serve` on an ephemeral port.
   - Records raw `POST /session` response and raw SSE `data:` payloads from `/event`.
   - Runs three scripted turns: normal text, tool lifecycle, failure path.
   - Writes fixtures to `rust/loopflow/src/lfd/sessions/harness/testdata/opencode_*.ndjson` plus `opencode_trace_manifest.json` (opencode version, capture date, scenario).

2. **Add OpenCode replay tests to `conformance_tests.rs`** mirroring Claude/Codex structure.
   - `replay_opencode_trace(file_name)` uses `opencode_mapping::ReaderState` and `map_event`.
   - Add at least three tests:
     - `opencode_trace_normal_turn`
     - `opencode_trace_tool_lifecycle`
     - `opencode_trace_error_turn`
   - Assert canonical event sequences and terminal turn status.

3. **Pin canonical schema in code** once fixtures are committed.
   - Replace fallback key scans in:
     - `opencode.rs::parse_session_id`
     - `opencode_mapping.rs::{session_id, map_permission, tool_id, tool_name/tool_status inputs as needed}`
   - If a required canonical field is absent, emit explicit error or skip with a debug log; do not silently try alternate names.

4. **Keep CI deterministic with committed fixtures**.
   - CI replays recorded fixtures only (no live OpenCode dependency).
   - Trace script is for maintainers refreshing fixtures when upgrading OpenCode.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep fallback parsing + unit tests only | Lowest effort now, highest latent risk later | Still masks schema drift and fails late in production |
| Run live OpenCode conformance in CI | Strong realism | Flaky/slow CI, external binary/network coupling |
| Add JSON Schema validator layer in front of mapper | Strong contracts | Extra abstraction before we know the real wire format; replay fixtures already give practical contract coverage |

## Key decisions

- **Record raw provider payloads, not mapped events.** This catches mapper bugs and proves schema shape.
- **Treat replay fixtures as contract tests.** This follows the Claude/Codex pattern and closes the OpenCode parity gap.
- **Be strict after schema confirmation.** Remove defensive multi-key fallbacks and fail loudly on unexpected payloads.
- **Success bet:** maintainers can upgrade OpenCode confidently by refreshing fixtures and seeing deterministic replay diffs.
- **Failure to avoid (6-month rip-out scenario):** only capturing one happy-path trace, then overfitting mappings to incomplete data. Mitigation: require text, tool lifecycle, and failure fixtures.

## Scope

- In scope:
  - OpenCode trace capture workflow and committed fixtures
  - OpenCode replay tests in shared conformance suite
  - Replacing fallback field-name parsing with canonical names proven by traces
  - Manifest documenting capture context (version/date/scenario)
- Out of scope:
  - Harness reconnect/backoff design
  - Immediate wave-advancement behavior changes
  - Broader session model or API changes

## Done when

- `cargo test -p loopflow conformance_tests` passes with new OpenCode replay fixtures.
- `rust/loopflow/src/lfd/sessions/harness/testdata/` includes OpenCode recorded traces + manifest.
- OpenCode mapping/harness code no longer uses defensive multi-key field fallbacks for schema-confirmed fields.

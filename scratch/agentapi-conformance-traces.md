# OpenCode conformance traces + schema pinning

## Scope

Harden the OpenCode harness by replaying provider traces in shared conformance tests and pinning runtime mapping to canonical OpenCode field names.

## Current state

This branch is in the implemented-and-reviewed state.

### Delivered

- Added OpenCode replay tests in `rust/loopflow/src/lfd/sessions/harness/conformance_tests.rs`:
  - `opencode_trace_normal_turn`
  - `opencode_trace_tool_lifecycle`
  - `opencode_trace_error_turn`
- Added replay fixtures in `rust/loopflow/src/lfd/sessions/harness/testdata/`:
  - `opencode_normal_turn.ndjson`
  - `opencode_tool_lifecycle.ndjson`
  - `opencode_error_turn.ndjson`
  - `opencode_trace_manifest.json`
- Added recorder script: `scripts/record_opencode_conformance_trace.py`.
- Removed defensive multi-key fallback parsing for schema-confirmed OpenCode fields in:
  - `opencode.rs::parse_session_id`
  - `opencode_mapping.rs` (session id, permission request id, tool id/state/name handling)
- Updated `wave/agentapi/02-hardening.md` to mark this hardening item complete and link here.

## Decisions

- Replay raw provider payloads (not mapped events) so mapper behavior is tested against wire shape.
- Keep CI deterministic by replaying committed fixtures instead of requiring a live OpenCode server.
- Use strict canonical keys for mapped fields; skip malformed/non-canonical events with debug logs rather than silently guessing alternate keys.

## Known follow-up

- Fixture provenance: in this environment, `opencode` was unavailable, so committed fixtures were hand-authored to canonical schema shape.
- Maintainer action when `opencode` is available:

```bash
uv run python scripts/record_opencode_conformance_trace.py
```

Refresh fixtures from live wire output and commit the updated trace files + manifest.

## Out of scope

- Harness reconnect/backoff redesign
- Immediate wave advancement behavior changes
- Broader session model/API redesign

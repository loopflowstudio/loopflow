# Branch review: OpenCode conformance traces + schema pinning

## What was implemented

- Added OpenCode replay coverage to the shared harness conformance suite in `conformance_tests.rs`:
  - `opencode_trace_normal_turn`
  - `opencode_trace_tool_lifecycle`
  - `opencode_trace_error_turn`
- Added OpenCode replay fixtures under `rust/loopflow/src/lfd/sessions/harness/testdata/`:
  - `opencode_normal_turn.ndjson`
  - `opencode_tool_lifecycle.ndjson`
  - `opencode_error_turn.ndjson`
  - `opencode_trace_manifest.json`
- Added `scripts/record_opencode_conformance_trace.py` to capture fresh fixtures from a live `opencode serve` instance.
- Tightened OpenCode schema handling to canonical keys only:
  - `opencode.rs::parse_session_id` now requires top-level `id`
  - `opencode_mapping.rs` now requires canonical `sessionID`, `requestID`, tool `id`, tool `state`, and tool `name` fields (with debug logging + skip behavior when missing)
- Updated wave tracking in `wave/agentapi/02-hardening.md` to mark this hardening item complete and link to `scratch/agentapi-conformance-traces.md`.

## Key choices

- **Replay raw wire payloads, not mapped events** to verify the mapper contract directly.
- **Pin to canonical schema now** instead of continuing defensive multi-key fallbacks that can hide provider drift.
- **Keep CI deterministic** via committed fixtures while keeping a recorder script for maintainers to refresh traces when upgrading OpenCode.
- **Fail soft in mapper** (skip + debug log for malformed/non-canonical events) to avoid mis-mapping bad payloads.

Alternatives rejected:
- Continuing fallback parsing (too much drift risk)
- Live OpenCode dependency in CI (flaky/slow/external coupling)

## How it fits together

`record_opencode_conformance_trace.py` captures one `/session` response plus filtered SSE payloads for three scenarios and writes NDJSON fixtures + manifest. `conformance_tests.rs` replays those fixtures through `opencode_mapping::map_event`, asserting canonical event sequences and terminal status. Runtime harness parsing (`opencode.rs` + `opencode_mapping.rs`) now shares the same canonical schema assumptions validated by replay tests.

## Risks and bottlenecks

- **Fixture provenance gap in this environment:** current fixtures were hand-authored because `opencode` binary is unavailable locally (`scratch/questions.md`). This is tracked, but maintainers should refresh fixtures from live OpenCode before relying on schema as fully observed.
- **Schema drift risk remains possible:** strict canonical parsing will intentionally drop unknown shapes; this is safer than silent fallback but means provider changes surface as missing events until fixtures + mapper are updated.
- **Recorder operational risk:** capture depends on `opencode serve` behavior and event timing; if upstream event ordering changes, recorder completion logic may need adjustment.

## What's not included

- No reconnect/backoff redesign for OpenCode SSE disconnects.
- No wave advancement behavior changes beyond existing tick-driven model.
- No broader session API/schema redesign outside the pinned OpenCode fields.
- No live OpenCode execution in CI; replay remains fixture-based.

## Wave alignment

- Advances `wave/agentapi` hardening goals by closing the OpenCode conformance replay gap and replacing schema fallbacks with pinned canonical fields.
- Leaves known hardening risk areas untouched (reconnect/backoff remains open).
- Observable metrics from this branch:
  - OpenCode now has replay tests for normal/tool/error scenarios in shared conformance suite.
  - `cargo test --all` includes passing OpenCode conformance + mapper unit tests.

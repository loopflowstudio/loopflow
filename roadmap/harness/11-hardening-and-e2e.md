# 11: Hardening + End-to-End Verification

Make the system production-ready for early adopters with failure handling and E2E validation.

## What exists after this

- robust timeout/iteration/failure behavior
- strong test coverage across Rust + Python + Swift + smoke path
- documented path for future explicit commit/push flow

## Commit slices

### C1 — Failure-path hardening (~300-500 LOC)

- enforce timeout + iteration stop behavior
- clear error propagation when final message missing
- client-visible failure semantics for dropped agent runs

### C2 — E2E tests + scripted smoke (~300-550 LOC)

- create wave, send chat, stream progress/final, inspect memory
- validate token-bounded history behavior
- validate ephemeral filesystem behavior

### C3 — Operational docs + explicit mutation stub (~250-450 LOC)

- document explicit apply/commit/push lifecycle (future-gated)
- add non-default guardrail path (feature-flag/off by default)

## Constraints

- Keep mainline chat turns read/think/write-memory first.
- Do not make implicit branch mutations in normal chat turns.
- Ensure CI coverage for all critical invariants.

## Done when

```bash
uv run pytest python/tests/ && cargo test --all && tests/e2e/test_smoke.sh
```

Expected: all relevant suites green for chat harness work.

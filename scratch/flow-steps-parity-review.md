# Flow steps parity and wave UX: consolidated review

## Scope

This file replaces two overlapping review notes:
- `jack-heart.bugs2.20260208_2138-review.md`
- `jack-heart.progress.20260208_2149-review.md`

It captures the current state of `flow_steps` support across API, Swift UI, and Python client.

## Current state

### Backend + UI (Rust + Concerto)

- Wave DTOs include resolved `flow_steps` from flow definitions.
- Running-wave UI can show per-step progress pills instead of only the flow name.
- Running-wave detail now includes commit log and diff stat while execution is in progress.
- Empty flow is treated as truly empty (no fallback display value).
- Run spawning uses shared helper plumbing (`spawn_run_task_with_slot`) across triggers and HTTP handlers.

### Python client parity

- `Wave` model now includes `flow_steps: list[str] = Field(default_factory=list)`.
- Payloads that omit `flow_steps` still parse safely.
- Model tests now verify:
  - default empty list for minimal payloads,
  - populated list parsing for full payloads,
  - round-trip serialization/deserialization fidelity.
- Python test payloads in `python/tests/conftest.py` are plain shared constants; unused pytest fixture wrappers were removed.

## Product impact

- Flow progress is now consistently available across clients that consume wave payloads.
- Python consumers can access the same flow-step data already used by Concerto.
- The UI is more honest for empty-flow waves and more informative during active runs.

## Known risks and tradeoffs

- Schema parity must still be maintained intentionally as DTOs evolve.
- Shared mutable test payload constants can create coupling if future tests mutate them.
- PR message generation in `auto_create_pr` is best-effort; failures degrade formatting but not core flow.
- Eager runs loading in UI increases API traffic, but with small payload cost.

## Not covered here

- No new CLI behavior changes.
- No additional migration work for removed postgres store testcontainers coverage.

---
linear_id: b00f983a-47b2-4ea8-b357-e45e0d183aa3
---
# Runtime Journal Protocol v2

Shipped. Implementation: `journal/mod.rs`, `lfd/journal.rs`, `lfd/types/event.rs`.

## Validate

```bash
# Journal round-trip: emit events, verify JSONL structure
cargo test -p loopflow journal::tests::journal_writes_run_flow_and_step_events_in_wave_worktree

# Daemon observation: lfd picks up journal events over websocket
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v

# Flow engine golden tests (covers event emission in flow execution)
cargo test -p loopflow golden_flows
```

# Refactor plan: wave workspace first, embedded terminals additive

## Validation

### Manual

Run:

```bash
uv run python scripts/concerto-dev.py run-debug
```

Verify:

1. Selecting a wave opens a work surface, not a terminal-only takeover.
2. Interactive waves still show the native session/chat path by default.
3. If a terminal session exists, the embedded terminal is available as an additive surface.
4. No-wave-selected still lands on the repo-wide queue.
5. Real attention items render correctly from backend data.

### Automated

```bash
swift test --package-path swift
cargo test --all
```

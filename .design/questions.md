# Open Questions

## From infra-engineer roadmap analysis (2026-01-21)

1. **Daemon error handling**: `server.py:314-315` swallows all exceptions in `_periodic_check`. Should this log errors? Surface them via the status API? This makes debugging daemon issues hard.

2. **Expanded lint rules**: Ruff is currently configured with E/F/W/I only. There are 30 blind exceptions (`BLE001`) and other issues. Worth expanding rules, or is the current minimal set intentional?

3. **Pre-commit hooks**: Should `ruff check` and `ruff format` run before commit locally, or is CI-only sufficient? Trade-off: faster local commits vs catching issues earlier.

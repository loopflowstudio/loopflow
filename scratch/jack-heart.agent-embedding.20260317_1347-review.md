# Review: jack-heart.agent-embedding.20260317_1347

## Validation

### Automated checks

| Suite | Result |
|-------|--------|
| `cargo fmt --check` | pass |
| `cargo clippy -- -D warnings` | pass |
| `cargo test --all` | 1 pre-existing failure (`wave_rename_renames_branch`) |
| `uv run pytest python/tests/` | 113 passed |
| `swift test --package-path swift` | 242 passed |
| `tests/e2e/test_smoke.sh` | pass |
| `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` | 16 passed |

### Manual product check

Run `uv run python scripts/concerto-dev.py run-debug`, start two waves with interactive steps, verify:
1. Selected wave opens work surface, not terminal takeover
2. Terminal tab appears when session exists, native chat is default
3. Terminal exit 0 resumes wave, non-zero fails run
4. No-selection shows attention queue
5. Attention items render with correct kinds

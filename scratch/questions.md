# Open questions / assumptions

- `opencode` binary is not available in this environment (`which opencode` returns not found), so the committed OpenCode trace fixtures were hand-authored to the canonical schema shape instead of captured from a live server. Once `opencode` is available, run `uv run python scripts/record_opencode_conformance_trace.py` to refresh fixtures with real wire data.

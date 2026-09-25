# CI proof after chapter changes

Run the architecture check after removing a public concept: retained tables and
subprocesses still need their actual owners in the checked map.

```bash
uv run python scripts/check_architecture.py
cd website && uv run python dev.py test -k 'portable_architecture or readme_index_sync'
```

Keep README and docs/index openings identical. Regenerate docs/architecture.html
when its source changes. For migration triggers, verify dependent behavior with
CI's materialized migration graph; an ordinary draft build may omit the trigger.
Task provider failures preserve Started history while releasing the worker claim.

Retiring Project surfaces also changes CLI fallback, builtin discovery, and prompt
goldens. Include those consumers and storage settlement tests in the Rust proof.
Regenerate prompt goldens with `uv run python tests/goldens/update_goldens.py`.
Run CLI-backed Python tests after the Rust build finishes: replacing their binary
mid-test mixes migration frontiers in a single temporary Home.

## Try it!

```bash
# New PM auth commands
lfq auth asana
lfq auth linear
lfq auth status

# Validation
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
```

What to look for:
- `lfq auth status` now includes **Asana** and **Linear**.
- PM API-key providers no longer show misleading `pay-per-token` copy in CLI status/output.
- Rust tests cover PM config parsing, wave `pm:` parsing, and roadmap `pm_id` round-tripping.

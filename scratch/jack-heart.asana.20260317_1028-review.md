# Validation

## Commands

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
```

## What to verify

- `lfq auth status` includes **Asana** and **Linear**.
- PM API-key providers do not show misleading `pay-per-token` copy in auth status/output.
- Rust tests cover PM config parsing, wave `pm:` parsing, and roadmap `pm_id` round-tripping.

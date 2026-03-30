# gstack stage 2: review validation

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
grep -R "gstack/projects" .lf/steps/gstack | wc -l  # expect 0
cargo run --quiet --bin lf -- --list
```

Expected `lf --list` output includes:
- `gstack-plan-manual`
- `gstack-review    [and] → gstack:review → gstack:cso → gstack:codex → gstack:review-synthesize`
- `gstack-sprint    gstack:office-hours → [xor] → ...`

Note: validate against the branch build (`cargo run --bin lf -- --list`), not a globally installed `lf`.

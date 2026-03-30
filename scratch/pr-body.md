## Try it!

```bash
cargo test --all
uv run pytest python/tests/
grep -R "gstack/projects" .lf/steps/gstack | wc -l
cargo run --quiet --bin lf -- --list | sed -n '30,40p'
```

What to look for:
- Rust and Python suites pass.
- The legacy `~/.gstack/projects/$SLUG/` path no longer appears in imported gstack step files (`0` matches).
- `lf --list` shows the new custom flows:
  - `gstack-plan-manual`
  - `gstack-review    [and] → gstack:review → gstack:cso → gstack:codex → gstack:review-synthesize`
  - `gstack-sprint    gstack:office-hours → [xor] → gstack-plan-manual → ...`

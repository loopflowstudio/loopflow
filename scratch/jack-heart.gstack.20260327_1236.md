# gstack stage 2: validation

```bash
cargo test --all
cargo fmt --check
cargo clippy -- -D warnings
uv run pytest python/tests/
grep -R "gstack/projects" .lf/steps/gstack | wc -l  # expect 0
cargo run --quiet --bin lf -- --list | grep gstack
```

## Done-when

- [ ] No gstack step references `~/.gstack/projects/$SLUG/` — all use `scratch/` or `.gstack/`
- [ ] Flow YAML files parse and chain gstack steps with loopflow builtins
- [ ] `and` constructs accept optional `synthesize` field
- [ ] `lf --list` shows gstack flows

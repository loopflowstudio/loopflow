# Performance

```bash
cargo build -p loopflow --bin lf
uv run python scripts/desktop_performance.py run --cli target/debug/lf --output /tmp/desktop-baseline
uv run python scripts/desktop_performance.py run --cli target/debug/lf --output /tmp/desktop-after \
  --baseline /tmp/desktop-baseline
```


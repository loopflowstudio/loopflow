## Try it!

```bash
cargo test --all
cargo test -p loopflow docker_
uv run pytest python/tests/
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift

sed -n '1,120p' docs/lfd.md
sed -n '1,120p' deploy/README.md
rg -n 'LFD_AUTH_MODE|LFD_AUTH_PROVIDER|mode: container' docker/docker-compose.yml deploy/docker-compose.prod.yml docs/getting-started.md docs/lfd.md
```

What you'll see:

- `cargo test --all` passes without depending on your real `~/.lf/config.yaml`
- the daemon, docs, and compose story is now `native|container` for deployment and `local|studio` for auth
- the default container recipe points at studio auth plus Docker without asking operators to choose unsupported install flags or old profile names

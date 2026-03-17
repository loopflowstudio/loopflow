# Bootstrap Wave Configs

## Validation

Run the focused Python checks for bootstrap behavior and exported API surface:

```bash
uv run pytest python/tests/test_bootstrap_redesign_script.py python/tests/test_api.py python/tests/test_cli.py -q
```

Expected result: all tests pass, including the bootstrap script checks for canonical repo-root resolution and redesign wave summary rendering.

## Try it

Bootstrap the redesign waves against an isolated daemon:

```bash
repo_root=$(pwd)
cargo build -p loopflow >/dev/null

tmp=$(mktemp -d)
git clone "$repo_root" "$tmp/loopflow" >/dev/null
rsync -a --delete --exclude '.git' "$repo_root/" "$tmp/loopflow/"
(
  cd "$tmp/loopflow"
  export HOME="$tmp/home"
  mkdir -p "$HOME"
  export LFD_HTTP_ADDR=127.0.0.1:2499
  export LFD_HOST=127.0.0.1
  export LFD_PORT=2499
  export LFD_DB_PATH="$tmp/lfd.sqlite"
  "$repo_root/target/debug/lfd" serve >/tmp/lfd-bootstrap.log 2>&1 &
  pid=$!
  trap 'kill $pid >/dev/null 2>&1 || true' EXIT
  until curl -sf http://127.0.0.1:2499/health >/dev/null; do sleep 0.2; done
  uv run python scripts/bootstrap-redesign.py
  uv run lfq show redesign
)
```

Expected result: all five redesign waves are created, and `lfq show redesign` reports `flow: tend`, `status: idle`, the four `wave/.../` area entries, and an absolute repo path.

# Branch Review — jack-heart.redesign.20260316_1213

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/ -q`

## Try it

Run the isolated-daemon bootstrap smoke check:

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

Expected result: all five waves are created; `redesign` reports `flow: tend`, `status: idle`, the four `wave/.../` area entries, and an absolute repo path instead of `.`.

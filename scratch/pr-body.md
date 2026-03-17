## Try it!

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/ -q
```

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

You should see five waves created, and `lfq show redesign` should report `flow: tend`, `status: idle`, the four member `wave/.../` paths, and an absolute repo path.

## Intent

Replace the legacy chord CRUD model with a single waves-only model, then bootstrap the redesign chord-wave on top of that. The branch removes the extra database/API/client surface, adds the redesign wave set on disk, and provides an idempotent bootstrap script that registers those waves through the normal wave API.

## Assumptions

- Chord membership is now represented exclusively by `area` entries pointing at `wave/<name>/` directories.
- There are no external consumers that still depend on `/v0/chords` or the removed Python chord helpers.
- Redesign waves should be created dormant first (`mode: manual`) and explicitly run later, once tend/build machinery is ready.
- The bootstrap script may be launched from a worktree, so it must resolve the canonical git common dir rather than storing `repo="."`.

## Key decisions

- Deleted chord-specific tables, routes, DTOs, and Python models instead of carrying a deprecated parallel abstraction.
- Introduced `wave/redesign/redesign.yaml` as the source of truth for chord-wave membership.
- Hardened `scripts/bootstrap-redesign.py` to register the canonical repo root and rely on wave configs for dormant/manual startup.
- Added regression coverage for bootstrap repo-root resolution, Python API exports, and `mode` loading from wave configs.

## Not included

- Tend flow steps, Letta integration, and mutation APIs.
- Chord-wave graph/UI work.
- Backwards-compatibility shims for removed chord endpoints.

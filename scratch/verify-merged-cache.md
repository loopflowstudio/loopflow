# LOO-921 verify (2026-11-22)

PR #2026 merged as 0b3e42bd41 (2026-11-23T04:07Z). Merge queue CI run 45108860832: success.
Push CI on main: run 45109945634 (event push), macOS on main 45109945701: success.

Live Machine, from a detached worktree at origin/main 0b3e42bd41:

- `uv run python scripts/build_cache.py install` → built/kept pinned `sccache 0.19.0` at
  `~/.local/share/loopflow/sccache/ca81ec2a…/bin/sccache`; `~/.cargo/config.toml` selects it,
  `jobs = 2` kept. Common-dir `hooks/loopflow-build-cache.py` now byte-identical to main's script;
  `post-checkout` keeps the lfd block after the cache block.
- `status` → daemon pid 90248 is the pinned binary; `103 of 103 loaded; this checkout loaded`.
- `git worktree add --detach` second worktree → hook registered it; `status`: new daemon pid 91052,
  `104 of 104 loaded; this checkout loaded`.
- `cargo build -p loopflow-storage` in the new worktree: 38.8 s wall, 49.9 s user, 9.6 s sys,
  peak RSS 1.13 GiB; Rust `200 hits, 4 misses` (only thiserror/thiserror-impl compiled).
- Both probe worktrees removed. `python3 -I scripts/build_cache.py status` with macOS system
  python fails (`tomllib`); documented command is `uv run`, so not a defect.
- Push CI run 45109945634 on 0b3e42bd41: success (2026-11-23T04:17Z).
- LOO-921 completed. LOO-922: baseline posted as Task report, checked out, `lf task run LOO-922 -a claude` launched from main with LF_*/CLAUDE* unset (pid 33688); status: moving.

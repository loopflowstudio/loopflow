# LOO-922 verify (2026-11-23)

PR #2028 merged as 4999d3cb27. Push CI run 45135080753: success. macOS on main: success.

On main (worktree at 4999d3cb, `scripts/build_cache.py install` refreshed the installed hook script, which was stale vs main):
- `git worktree add --detach ../loopflow.verify922-seed HEAD`: 2.4 s; `status` → `seed from ../loopflow.43bf9b-remove-chapters-krs at 4d845ffd21`.
- `cargo build -v -p loopflow --bin lf`: 7 Compiling, 249 Fresh, 161 s real under load avg ~155, peak RSS 3.56 GB. `./target/debug/lf --version` → `lf 0.16.0+4999d3cb2` (own executable, not sibling's).
- Worktree on an HFS+ sparse image (no clone across volumes): no `target/`, `status` → `seed none`. Cold fallback clean.

Not run: Linux reflink volume and ext4 fallback → follow-up 4694cd. LOO-923 already done (#1978); nothing to launch.

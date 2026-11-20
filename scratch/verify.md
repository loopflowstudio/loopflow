# LOO-938 verify (main 504e20b4, 2026-11-20)

Outcome: Verified, plus one follow-up (34bc76).

Built main `lf` in a detached worktree, with isolated HOME/LF_HOME per Machine and a bare local remote.
- A: `lf task create 'Verify plan travels'` → `78fafc`; `git ls-remote` showed `refs/loopflow/plan/home_448e…` right away.
- B (clone): `lf task list` → `78fafc · — · Verify plan travels`; `lf repo tracker` → `Plan: Git origin.`; B's own ref appeared.
- B: `lf task edit 78fafc --title 'Edited on B'`; A: `lf task sync 78fafc` → `nothing unsent`, `lf task list` → `Edited on B`.
- Gap (existed before this PR): when `lf task sync 78fafc` is the first command on a fresh home, it fails with `no Loopflow registry on this machine; run lf doctor`. Follow-up 34bc76.
- Not run here: hub over real SSH (PR says so), first-wins claim, Wave split.
- CI: merge_group CI success. The push CI is still running and "macOS on main" is queued for 504e20b4. Both are pending, not passed.

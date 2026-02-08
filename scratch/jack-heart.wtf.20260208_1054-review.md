# Wave worktree visibility and upstream sync

## What was implemented

Three related improvements to wave worktree management:

1. **Worktree/branch fields on Wave DTO** — `local_worktree` and `remote_branch` are now surfaced on the Wave API response (Rust DTO, Python model, Swift model), inferred from disk at query time. The CLI table and Swift `WaveViewModel` fall back through active run → wave-level fields → defaults.

2. **Background upstream sync** — `git push -u origin <branch>` is no longer blocking. `schedule_upstream_sync` spawns a background thread with exponential backoff (0s, 2s, 5s, 15s, 30s, 60s) and bails early if upstream is already set. Called both on worktree creation and on reuse (in `ensure_wave_worktree`). Auth prompts are suppressed via `GIT_TERMINAL_PROMPT=0` and `GCM_INTERACTIVE=Never`.

3. **Worktree path resolution from worktrees** — `worktree_path()` now resolves through `main_repo_root()` so that calling it from inside an existing worktree produces the same sibling path as calling from the main repo. Prevents nested `.repo.seed.beta` paths.

4. **Optimistic wave creation in Concerto** — `RepoState.createWave` inserts a pending `WaveViewModel` immediately, swaps it for the real one on success, and rolls back on failure.

## Key choices

- **Infer git state at query time** rather than persisting it in the store. Worktree paths and branches change (renames, next-iteration resets), and the filesystem is the source of truth. The `spawn_blocking` in `build_wave_dto` keeps the async handler non-blocking.

- **Fire-and-forget thread for upstream sync** rather than async task. The sync is best-effort — if it fails after 6 retries (~112s total), the worktree still works locally. No error propagation needed.

- **`main_repo_root` via `git rev-parse --git-common-dir`** — reliable way to find the main repo from any worktree. Falls back to the input path on error (non-git directories).

## How it fits together

The HTTP layer (`build_wave_dto`) calls `infer_wave_git_state` in a blocking task to check if a worktree exists for a wave and what branch it's on. This data flows through the DTO to the Python CLI (table columns) and Swift app (fallback chain in `WaveViewModel.init`). Separately, `schedule_upstream_sync` ensures new branches get pushed without blocking worktree creation.

## Risks and bottlenecks

- **`main_repo_root` in `worktree_path`** — adds a `git rev-parse` subprocess call on every path computation. In practice this is fast (<5ms) and only called a handful of times per request. If it becomes hot, memoization per repo path would fix it.

- **Background thread leak** — `schedule_upstream_sync` threads are detached. If many waves are created rapidly, threads could accumulate waiting on retries. The 60s max backoff and early-exit on success bound this in practice.

- **`spawn_blocking` per wave in list endpoint** — listing N waves makes N blocking calls to check worktree state. For typical wave counts (<20) this is fine. At scale, batch the checks.

## What's not included

- No persistent tracking of sync status (whether upstream push succeeded).
- No UI indicator for "push pending" state in Concerto.
- No batching of `infer_wave_git_state` calls in the list endpoint.

# Open questions

## A2 step 7 — `Wave::repo()` accessor retained

The base (e65526ce) had already removed the flat `Wave.repo`/`status`/`iteration`/
`cycle_start_iteration` fields and `primary_repo()` (ancestor 8228a5dd), plus the
migration and store work. What remained was a repos-backed `Wave::repo()` accessor
(`self.repos.first().map(|r| r.repo.as_str()).unwrap_or("")`) used by ~27 single-repo
readers.

Decision: kept `repo()` as the single sanctioned "primary repo" accessor. It centralizes
the "single-repo need → `wave.repos.first()`" rule the task prescribes rather than
inlining that Option chain across every call site (which would be more verbose and
riskier than the accessor). Repo-*filter* sites were repointed to
`wave.repos.iter().any(|rw| rw.repo == repo)` per the multi-repo membership semantics.

If the reviewer wants zero single-repo bridge at all, inline `repo()` at its call sites
and delete the accessor.

## Demo session findings (2026-07-03)

Ran `scripts/concerto-dev.py run-debug --with-lfd` to demo the `repos:[RepoWork]`
surface. The model works end-to-end: every seeded wave came back with a fully
populated `repos:[RepoWork]` (status, `local_worktree`, `remote_branch`, commits,
diff_stat, stack_count) read live off sibling worktrees. Three findings:

1. **FIXED — empty-body JSON POST 400.** `LocalWaveService.makeRequest` set
   `Content-Type: application/json` even when `body == nil` (the no-overrides
   `waves/{id}/run` case). axum's `Option<Json<T>>` rejects an empty typed body
   ("EOF while parsing"), so starting a wave's `/goal` agent 400'd and the pane
   showed "Terminal unavailable." Fix: only send the content-type when a body is
   present. Verified: `/run` now 200s and launches.

2. **Dev-mode goal-resolution split (environment, not an A2 defect).**
   `launch_wave_agent_session` resolves the goal from the *conventional* worktree
   `worktree_path(main_repo_root(wave.repo()), name)` → `loopflow.<wave>`. But the
   goals were authored in this dev checkout's `wave/` dir (surfaced to Concerto via
   `CONCERTO_DEV_WAVE_REPO`), and the main-derived siblings `loopflow.website/.release/
   .concerto` are stale unrelated June wave-branches with no `wave/<name>/GOAL.md`
   → "goal not found." Only `loopflow.goals` happens to carry its goal file, so only
   `goals` launches+attaches cleanly (verified: HTTP 200, tmux session live, full
   goal prompt inlined). Open question: should launch trust `RepoWork.local_worktree`
   instead of recomputing by naming convention? (Wouldn't fix website — its
   local_worktree also lacks the goal — so the deeper gap is dev-checkout goals not
   reaching the launched worktree.)

3. **`--with-lfd` runs a redundant daemon.** It seeds/runs a full autorunning lfd on
   :2486 that Concerto ignores (Concerto uses its own bundled daemon on an ephemeral
   port). The :2486 loop-ticker then throws `goal not found` autorunning the seeded
   waves. For demoing this surface, plain `run` (bundled daemon only) is cleaner.

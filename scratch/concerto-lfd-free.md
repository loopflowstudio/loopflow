# Concerto lfd-free basic UX (ship slice)

**Goal:** Concerto lists waves, launches a wave's `/goal` loop in a tmux session,
and attaches to it — through `lf`, with lfd out of the launch/attach path (today
lfd is only a name-resolver there). The bar is the normal flow working well: open
Concerto, click a wave on loopflow/Cadenza, get a working goal terminal.

Background + rationale: `scratch/lf-unification.md`. This brief is the buildable
slice; everything else in that doc is deferred to the reduce roadmap.

## Done when

1. `lf goal <wave> --tmux` (run from any checkout of the repo):
   - ensures the wave's worktree `../<repo>.<wave>` exists (creates it if missing),
   - starts the goal loop in a detached tmux session **whose cwd is that worktree**,
   - prints the tmux session name (the handle) and exits 0,
   - is idempotent: a second call with the session live just reprints the handle.
2. In Concerto, clicking a wave launches + attaches its goal terminal (the normal
   flow), and the wave list renders from disk — no lfd query in that path.
3. `tmux ls` shows the session named `lf-<repo>-<wave>` running in `../<repo>.<wave>`.

## Task 1 — `lf goal --tmux` runs in the wave's worktree (Rust)

File: `rust/loopflow/src/lf/commands/goal.rs`, fn `launch_in_tmux` (already exists;
today it runs in `main_repo` and names the session `<repo>-<wave>`).

Change it to:
- Resolve `worktree = engine::worktrees::worktree_path(main_repo, wave_name)`
  (`= ../<repo>.<wave>`).
- If `worktree` does not exist on disk, create it:
  `engine::worktree::create_worktree(main_repo, &worktree, &branch)` where `branch`
  is the stable `<author>.<wave>` (reuse the author/branch naming in
  `engine::naming`; do NOT timestamp — the worktree must be deterministic). If it
  already exists, reuse it as-is regardless of its current branch.
- Name the session after the **worktree basename**:
  `tmux_session_name(worktree.file_name())` → `lf-<repo>-<wave>`.
- Launch tmux with `-c <worktree>` (not `main_repo`) and run the inner
  `lf goal <wave>` there. Keep the existing idempotent `tmux has-session` check and
  the login-shell wrapping.

Note: `lf goal` (the inner, non-tmux path at the top of `run`) currently also
collapses to `main_repo`. Leaving that is fine for this slice — the tmux wrapper
sets cwd to the worktree, and the inner `lf goal` re-resolves from there.

## Task 2 — Concerto wave list renders from disk alone (Swift)

File: `swift/Concerto/Platform/macOS/Views/WavesView.swift` (the `mergedWaves` /
`authoredPlaceholder` path already reads `<repo>/wave/<name>/GOAL.md`).

Ensure the list renders with **no lfd connection**: the authored-from-disk waves
are the baseline; lfd live-waves are an optional overlay, not a requirement. When
lfd is absent, show the disk waves with status derived from `tmux has-session`
against the deterministic handle (D3), not "disconnected/empty".

## Task 3 — Click → `lf goal --tmux` → attach by name (Swift)

Files: `swift/Concerto/Platform/macOS/Views/WavesView.swift` (~672, ~890),
`swift/Concerto/State/PortfolioRepoState.swift` (`startWaveAgent`).

Today the click does `POST waves/{id}/run` + `attachSession(id)` (both lfd). The
attach itself is already plain tmux (`tmux attach-session -t <name>`,
`WavesView.swift:890`, `TerminalWorkspaceView.swift:186`) — lfd only *resolves the
name*. Replace the lfd calls with:
- Exec the **bundled** `lf` (same binary Concerto bundles) as
  `lf goal <wave> --tmux` with cwd = the repo.
- Read the printed handle from stdout.
- Feed that handle to the existing `tmux attach-session -t <handle>` attach view
  (reuse `GhosttyTerminalView`); skip `attachSession(id)`.

Concerto can also *derive* the handle for a wave (same rule as Task 1:
`lf-<repo>-<wave>`) to show running state and re-attach without launching.

## Tests (the wider notion of "works")

The demo bar is the normal flow above. Breadth and failure modes live in tests,
not the demo:

- **lfd absent:** with no lfd reachable, `lf goal <wave> --tmux` still launches and
  prints a handle, and Concerto's list + click-launch still work. (This is the
  "kill lfd, still succeeds" scenario — a test, not a demo stunt.)
- **Idempotent relaunch:** a second `lf goal --tmux` with the session live reprints
  the handle; no duplicate session.
- **Create-if-missing:** fresh repo with no `../<repo>.<wave>` → the worktree is
  created and the loop runs there.
- **Stale handle:** `lf-<repo>-<wave>` recorded but the tmux session died → relaunch
  recreates cleanly.

## Out of scope (defer to reduce roadmap)

`lfdb` extraction · `lf`-self-registration into sqlite + "show active agents by
worktree" · `lf d`/`lf q` namespaces · deleting lfd's HTTP executor (the hard cut)
· subscription-based live status · Concerto proactive worktree pre-allocation.

This slice leaves lfd running (it still serves subscriptions/live status); it's
just out of the launch/attach path. No irreversible cut required.

## Reuse map (don't reinvent)

- Worktree path: `engine::worktrees::worktree_path`
- Worktree create: `engine::worktree::create_worktree` (see `flow.rs:475`)
- Branch/author naming: `engine::naming`
- tmux session name: `lfd::types::tmux_session_name`
- Attach view: `GhosttyTerminalView` + `tmux attach-session -t <name>`

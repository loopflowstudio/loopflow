# Open questions & assumptions

## update-wave pass (2026-07-03)

- **Executive decision: `reduce` is a new standalone wave, not folded into
  `goals`.** The `lf-unification.md` roadmap (collapse `lfd`/`lfq` → `lf`, shrink
  lfd to a subscription server) was seeded on this branch (commit "seed reduce
  wave"). It's a distinct concern — net-negative plumbing collapse — from goals'
  Looping-Agent vision, so it gets its own home at `wave/reduce/`. It's
  standalone (no chord references it yet); the chord can pick it up later.
  Reversible if review prefers it merged into `goals`.
- **Shipped items deleted:** `concerto/5-reshape-and-trim-surface` (WavesView is
  the surface, dead UI trimmed — native-chat views confirmed absent from source).
- **Obsolete item deleted:** `concerto/02-native-chat-ux` — the branch's "frame,
  don't render / zero native chat surfaces" direction deleted that stack outright;
  a polished native chat contradicts the wave. Coherence deletion, not a shipped
  one.

## Live open questions (folded into items)

- **Goal resolution in the launched worktree.** `lf goal --tmux` resolves the
  goal from the main-derived sibling `loopflow.<wave>`, not
  `RepoWork.local_worktree`; dev-checkout goals surfaced via
  `CONCERTO_DEV_WAVE_REPO` don't reach it → "goal not found." Should launch trust
  `local_worktree`? (Wouldn't fully fix it — the deeper answer is lfd-owned
  identity materializing GOAL/MEMORY into the launched worktree.) Folded into
  `wave/concerto/1-embedded-terminal-build-driver.md` and
  `2-lfd-owned-wave-identity.md`.
- **`lf`-launched sessions invisible to lfd live status.** `lf goal --tmux` does
  not register its session. Folded into `wave/reduce/2-session-registry.md` and
  the concerto embedded-terminal item.
- **A2 step 7 — `Wave::repo()` accessor kept.** Kept the repos-backed
  `repo()` accessor (`self.repos.first()…`) as the single sanctioned "primary
  repo" reader rather than inlining the Option chain across ~27 call sites;
  repo-*filter* sites moved to `repos.iter().any(...)` membership. If review wants
  zero single-repo bridge, inline `repo()` and delete the accessor. Shipped
  decision, noted for the reviewer.

## Dev-tooling note (not a defect)

`scripts/concerto-dev.py run-debug --with-lfd` seeds/runs a full autorunning lfd
on :2486 that Concerto ignores (it uses its own bundled daemon on an ephemeral
port); the :2486 loop-ticker then throws `goal not found`. For demoing the
lfd-free surface, plain `run` (bundled daemon only) is cleaner.

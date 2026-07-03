# Terminal-first Concerto — driving roadmap

The doc `lf` iterations read to move toward the goal. Keep it short and current;
record what you learn in the wave's MEMORY, not here.

## Goal (behind `/goal`)

Concerto becomes the terminal-first surface that frames waves — **portfolio →
repo → wave** — on a **Wave/RepoWork** model that splits a wave's singular
identity (GOAL, MEMORY, agent, flow) from its per-repo execution (worktree,
branch, run, PR). It hosts the vendor's TUI sessions in embedded panes and never
renders chat itself. Each iteration: the smallest change that makes the model or
a screen more true, without stranding the build.

## Where we are (honest)

- **Backend A2 (Wave.repo → repos:[RepoWork])** — Steps 1–2 landed, committed,
  green (RepoWork type, `wave_repos` table + store methods, `Wave.repos`,
  `primary_repo` bridge, repos stitched on read/write). Steps 3–7 remain. Detail:
  `scratch/a2-plan.md`.
- **The miss:** the Concerto UI was **not** emptied/reset. It still shows the old
  wave-workspace (a sidebar of *waves* + TERMINAL / MARKDOWN-README / ROADMAP
  panes). We refactored the model but didn't reset the surface.

## Hard constraint

**Nothing commits until SOME new minimal UI exists.** The deliverable of this
increment is the reset surface, not more backend. Backend A2 resumes *after* the
minimal UI is real.

## Next moves — UI reset first

1. **Empty + reset Concerto to the minimal surface.** Route the main window to a
   blank-slate view: **left sidebar = repos** (from the repo registry, select to
   filter) + **center = the list of waves** (filtered by selected repo; row =
   name · rollup status · repo chip). Nothing else. Unwire the old
   wave-workspace / multiplexer / detail / README+roadmap panes from the exposed
   path (keep files in tree — `git` is history). **This is the commit gate.**
2. **Wire it to real data.** Waves from `RepoState`; repos from the registry;
   filter on `wave.repo == selectedRepo` today (swap to
   `wave.repos.contains(repo)` after A2 lands). Build green (Concerto compiles).
3. **Resume backend A2** (Steps 3–7 in `a2-plan.md`) to make the filter honest —
   `repos:[RepoWork]`, status rollup, DTO nesting.
4. **Wave screen** (terminal-first: goal-loop harness pane + yazi over
   GOAL/MEMORY/scratch + ad-hoc terminals + RepoWork strip) — later slice.
5. **Quick-start** a wave from repo roadmap / Asana — later slice.

## Then — the loop experiment

Launch a goal-driven agent per wave: initial prompt `/goal <contents of
wave/<wave>/GOAL.md>`, based on `main`, one per repo. Validate the loop
end-to-end (agent reads its GOAL, picks a move, dispatches a worker, folds into
MEMORY). Depends on `lf op roadmap` (see `scratch/roadmap-ops-migration.md`) once
that lands; for now GOAL/MEMORY files drive it.

## Guardrails (from the design)

- Frame, don't render — no native chat surfaces.
- GOAL/MEMORY are singular wave identity, above the repo fan-out.
- Repo is a filter, not a container.
- Drop = unwire from the product, keep the file in tree.

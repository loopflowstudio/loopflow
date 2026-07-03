---
primary_flow: build
pm:
  provider: asana
  asana_project: '1214270017631632'
---

Concerto is the terminal-first surface that frames waves — repo → wave — hosting
the vendor's TUI sessions in embedded panes, never rendering chat itself. Build it
by reshaping the battle-tested surface into the shape we want, not by rebuilding
fresh alongside it.

**Metrics to improve**
- Daily wave work done in Concerto vs. Terminal.app — toward "the daily driver."
- Wave navigation reads cleanly off the Wave/RepoWork model, not per-repo hacks.
- "Frame, don't render" held — zero native chat surfaces.
- UI trimmed to what the exposed views actually use — reuse proven components over
  rebuilding; net-negative code each pass.

**Milestones**
- `Wave.repo` → `repos: [RepoWork]` lands green through store, executor, DTOs, and
  fixtures.
- The surface is `WavesView`: a burgundy repo sidebar filtering a wave list, a
  new-wave launcher, and clicking a wave opens its `/goal` agent in an embedded
  tmux terminal — reshaped from proven components, dead UI trimmed away.
- lfd owns wave identity: GOAL/MEMORY master, export on PR, pull-in on subscribe.
- Wave-screen UX explored (2–3 variants via `lf ux-research`) and a standing UX
  iteration loop.

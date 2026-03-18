# Backlog

Ideas worth revisiting after the redesign waves stabilize. Not prioritized, not scheduled.

## scale/01 — FlowRun Container

WaveRun/FlowRun split: iterations own branches/PRs, flows execute within them. Real architectural need but the shape gets clearer after chord-model ships and tend cycles run. Revisit when the pain is concrete.

## scale/04 — Chords UI

Chord sections in sidebar, visual grouping, listen indicators, CRUD from UI. Needs the chord model to stabilize first. Natural follow-on to agent-embedding once there's something to render.

## concerto/04 — Auto-Send

Auto-send on VAD silence with confidence-based behavior and continuous conversation toggle. Polished interaction pattern, but a refinement on top of voice features that aren't the priority.

## concerto/03 — Release UI

Per-repo release config (cron toggle) and "Release Now" button with version picker. Nice quality-of-life, but `lf ops release` from CLI works. Save until Concerto has more fundamental workflows.

## context/01 — Direction Aliases

User-defined direction aliases (`designer` → `[ux, craft, aesthetics]`) stored in lfd sqlite. Clean ergonomic win, small scope. Worth doing eventually but doesn't unlock anything.

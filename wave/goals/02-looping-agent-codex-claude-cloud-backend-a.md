---
priority: high
asana_id: '1216257840999672'
---
# Looping Agent — codex/claude cloud backend (a)

**Finish line:** From Concerto (or `lf`), launch a per-repo looping session in codex/claude's cloud that re-runs a Goal prompt on their native schedule, with Asana wired in, and deep-link back to it.

## Context

Backend (a) is **first** — *"work well with claude and codex so we're not asking you to change your workflow; we adapt to yours."* It extends the committed vendor-handoff thesis (2026-06-19): steps are Skills, runs hand off to the vendor session via a small `/step`/`$step` seed. A Goal is the looping version: the vendor's own recurring trigger re-runs the goal prompt; loopflow provides the rendered prompt + Asana wiring, not the runtime. We rent persistence.

## What to shape

- Render a Goal into a vendor-launchable loop (goal prompt + roadmap access).
- Two sub-options to pick at build time:
  - **A1** — lfd drives the vendor cloud API/CLI: launch a remote session + register a recurring trigger that re-invokes the goal. Concerto button → live cloud loop. More magic, more coupling to moving vendor APIs.
  - **A2** — lfd scaffolds (prompt, Asana MCP, loop instruction, deep-link); the human presses go in codex/claude. Thinner, respects "your workflow" hardest, but lfd doesn't own the lifecycle.
- Deep-link back out so the session is re-accessible in the vendor UI.

## Done when

- One repo's Goal runs as a recurring vendor-cloud loop, reads its Asana roadmap, and is reachable via a Concerto deep-link.

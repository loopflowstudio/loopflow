---
workers: 0
pm:
  provider: linear
  linear_project: '9ee88f2a-ef37-46c7-b201-d197db3ccae0'
---

## Objective

Run one loop iteration for the Concerto wave.

You are Concerto for macOS — the layer *above* the vendors' sessions, not a chat
client. Concerto frames the work; the vendor (Claude Code, Codex, opencode) runs
it inside an embedded terminal pane. You keep sessions alive across restarts and
show wave state around them; you do not reimplement the vendors' chat.

Read the roadmap, judge the Concerto surface against the metrics, and pick the
next useful move: harden terminal launch and reattach, tighten multi-agent
dispatch, make wave state legible around a live session, or polish the window
into one daily surface. Dispatch the appropriate flow against it. The north: a
conductor opens the app and the vendor's own session is right there — framed,
never re-rendered. If no safe move remains, record the blocker instead of
inventing work.

## Measures

- **Key Results**: an embedded vendor session survives an app restart and reattaches cleanly.
- **Key Results**: launching a session is one action from the wave view.
- **Quality**: Concerto renders no assistant turns itself — the vendor's TUI owns the conversation.
- **Quality**: wave state is visible around every live session, not buried a click away.
- **Done means**: a landed PR of real product code, roadmap item closed and PR-linked.

## Process

Read the live roadmap, judge the surface against the measures, and dispatch the
appropriate flow for the next useful move. Routing is prose judgment, not
frontmatter.

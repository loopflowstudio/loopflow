---
description: Operate the Wave — read, decide, and take the one or two useful moves in a single turn.
action_style: procedural
---
Operate the Wave in one turn: understand where it stands, choose the one or two
useful moves, and take them.

## Your final message is posted to a channel a person reads — so usually say nothing

Your visible reply is delivered verbatim to the Wave's chat channel. Treat it
like a message to a busy person, not a work log. **Most passes should end with no
message at all.**

- **Silence is the default.** A routine tick, a re-read that found no change, an
  attempt that hit the same known blocker — end the turn with an **empty reply**.
  Do not write "nothing changed," "yielding," "no action needed," or a recap.
  Empty output is not delivered; that is correct.
- **Speak only when there is genuine news** the person can act on: work you
  actually started or finished (with its link), a blocker you are surfacing for
  the first time (one line, with the exact fix needed), or one sharp question.
- **Never narrate your machinery.** No "I'm using wave/operate," no phase or
  skill or flow names, no "X is retired," no play-by-play of launch attempts, no
  control-plane vocabulary (ancestry, placement, durable Turn Basis, reservation).
  The person does not care how you work.
- **State a blocker at most once.** If you already reported it on an earlier
  pass and nothing changed, stay silent this pass. Do not re-post it every tick.
- Do your reading and reasoning silently; the channel hears only a deliberate,
  plain sentence or two, or nothing.

## The hierarchy is an intent graph, not a control plane

Wave → Project → Task is a graph of intent and purpose: it makes work visible,
gives intent a shareable shape, and ties code to the work it serves. It is **not
ownership and not permissions.** It gates nothing. An unhealthy Project or Task
agent never blocks you — adding a Task is just adding a node to the graph.

- **Always launch work.** If work should start, start it. Do not wait for a
  Project agent, a placement check, or a valid ancestry. If a reader or the Task
  controller is down, do the computable thing directly.
- **Step in when a delegated layer fails.** Project agent can't create the Task?
  Create and run it directly. A Task run crashed mid-work? Inspect it with `lf
  runs` and finish the work in that Task's worktree (concurrent writers are fine).
- **Capture stays.** Work is still captured as Task nodes with Project placement;
  only the requirement that the ancestry be *healthy first* is gone.

## Read, decide, act

Resolve the exact Wave from the prompt or `wave/<wave>/GOAL.md`. Read GOAL/MEMORY,
the recent human conversation, the cache-only PM snapshot, and current
Project/Task state. If a reader fails, work from memory — repairing PM or auth is
not the objective. Trust worker summaries; do not reread transcripts.

- Answer a waiting human first, plainly and with the useful thing.
- Select from filed Tasks and open KRs, or file the Task the moment work should
  start. Start it with `lf task run <issue-id> --directive "<brief>"`; if the
  controller is down, create and run the node directly.
- Supervise with `lf task status/steer/interrupt/wait/resume`. Independent Tasks
  run in parallel; never a second session for one issue.
- Keep coordination and small read-only decisions in the Wave.

When reporting more than one Task, read `lf roadmap --wave <exact-wave> --json`
and `lf status <exact-wave> --json` and render each as
`[identifier](provider URL) — readable active PR/workspace slug — status/next owner`,
filling the link from
`task.identifier`/`reference.issue_url`, the slug from `active_pr.slug` (fall back
to `reference.workspace.slug`), status from `runtime.status` or the roadmap
`section`, next owner from `next_move.owner`. Never reconstruct a provider URL,
branch, or slug from an identifier or title. Omit only a link or slug whose
snapshot evidence is explicitly absent; keep the Task and its status/next owner.

## Uncertainty selects the flow, it never blocks

Do not stop to ask permission before launching. If uncertain, launch the work
with a flow whose lifecycle already contains a human review gate (`task-design` →
`task-gate`, or a `finally` review) rather than blocking the channel. Confident
work runs a straight-through ship flow. Your only judgment is *which flow*.

Correct `GOAL.md` only when the objective, bounds, or cadence no longer ask the
honest question; correct Project definitions or KRs through `lf pm project
update`. Promote a durable operating context into a Wave, never a child Project.

Keep the turn to the one or two useful moves. The Wave runner advances the flow;
write no loop bit.

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

Wave → Task is a graph of intent and purpose: it makes work visible,
gives intent a shareable shape, and ties code to the work it serves. It is **not
ownership and not permissions.** Task motion comes from durable
intent and explicit operations, not resident ancestor processes. Only Tasks
persist a selected Flow position.

- **Always launch work.** If work should start, start it with `lf task run`; the
  exact Task Flow-position claim collapses concurrent nudges.
- **Recover through Work.** A crashed boundary Run is replaced from durable
  state. Helper Runs may assist in the Task worktree, but never drive its Flow.
- **Capture stays.** Work is still captured as Task nodes with automatic chapter placement;
  only the requirement that the ancestry be *healthy first* is gone.

## Read, decide, act

Resolve the exact Wave from the prompt or `wave/<wave>/GOAL.md`. Read GOAL/MEMORY,
the recent human conversation, the cache-only PM snapshot, and current
Wave objective, chapter KRs and metric targets, and Task state. If a reader fails, work from memory — repairing PM or auth is
not the objective. Trust worker summaries; do not reread transcripts.

- Answer a waiting human first, plainly and with the useful thing.
- Select from filed Tasks and open KRs, or file the Task the moment work should
  start. Start it with `lf task run <issue-id> --directive "<brief>"`.
- Supervise with `lf task status/steer/interrupt/wait/resume`. Independent Tasks
  run in parallel; never a second session for one issue.
- Keep coordination and small read-only decisions in the Wave.

When reporting more than one Task, read `lf roadmap --wave <exact-wave> --json`
and `lf status <exact-wave> --json` and render operational rows as
`[identifier · Task title](provider URL) — status; next action/owner`.

Use this ID-first form in operational lists. In prose, use
`[Task title · identifier](provider URL)` on first mention; shorten later
references when unambiguous.

Fill the link from `task.identifier` and `reference.issue_url`, and the readable
title from `task.title`. Take status from `runtime.status` or the roadmap
`section`, and next owner from `next_move.owner`. State the next action only
when supported by current evidence; leave unknown state unknown. Include an
active PR/workspace slug only when navigating that workspace is the job. In
roadmap, use `active_pr.slug`; in status, match `active_pr` to `prs[].id` and
use that PR's `slug`. Fall back to `reference.workspace.slug`. Never infer a
URL, branch, or slug from a title or identifier. If a link is absent, keep the
readable title and available status without inventing a URL.

## Recover or contribute to existing work

Use `lf task prepare <issue> --json` to prepare without starting execution.
A bounded `lf --task <issue> research "<question>"` runs in its existing worktree
without advancing the Task Flow or acquiring a mutation lease. Give independent
contributors distinct scratch paths, wait for the needed artifacts and inspect
them before directing the Task. Contributors leave edits uncommitted. Do not
checkpoint another active contribution's dirty files.

If new evidence invalidates the current attempt, update the Task definition,
wait for required contributions, then use
`lf task restart <issue> "<changed direction>"`. It checkpoints and pushes the
current tree, preserves identity/worktree/PR history, interrupts the exact Task
worker if live, and starts the chapter's recommended Flow fresh. Reconcile old
scratch as evidence. For dependent Tasks use `--stack-on <parent-task>`; each
Task keeps one active PR. Honor explicitly selected Flows.

When execution seems stuck, inspect `lf top` or `lf ps --json` before guessing.
Idle time alone is not failure; never kill an unclaimed provider PID. Placement
is durable: inspect `lf status <wave> --json`, change it through
`lf work place wave <wave-id> <home-id>` only when that is the intended action,
and use `lf ssh <home-id> start <wave>` to start at a remote placement. Do not
launch a competing local worker to work around an unavailable Home.

## Uncertainty selects the flow, it never blocks

Do not stop to ask permission before launching. If uncertain, launch the work
with a Flow that contains the needed human review gate, such as `task-design`,
rather than blocking the channel. Confident work may use a straight-through
Flow. Your launch judgment is *which Flow this Task worker should run*.

Correct `GOAL.md` only when the objective, bounds, or cadence no longer ask the
honest question; correct chapter metric targets or KRs through `lf wave update-plan --wave <wave> --plan <plan.json>`. A Wave has exactly one current chapter plan. Never create parallel Projects or a second operator tier.

Keep the turn to the one or two useful moves. Each invocation acts once; write
no loop bit.

## Judge the chapter evidence

Check a KR only when its observable condition and full duration already hold.
For each sponsored metric that moved, decide outcome Task, instrument repair,
wait, or no action. A Met frontier may keep a worker; a Met guardrail stays quiet until its alarm.
Read filed Tasks before creating work. Use `lf pm task create --wave <wave>`
or `lf task start --wave <wave> <title>`; chapter placement is automatic.
A chapter boundary uses `lf wave new-chapter`, never ad hoc backlog cleanup.

## Task briefs

Write the Task from the user's perspective. Explain what they're trying to do, what
gets in their way, and why it matters. Give it a title naming the problem or desired
experience. Show what success would look like in a concrete moment of their work.
Preserve the user's own language when it anchors intent.

Ground the Task in observations, real constraints, and examples of success. Possible
solutions can help explain the idea; mark what remains uncertain. The design doc develops
the architecture, APIs, implementation sequence, and verification. A Task is ready for
design when the problem is clear, even if the solution isn't. Link accepted decisions
and keep them binding.

Keep the Task useful to someone choosing what to work on now. As understanding changes,
update the problem and desired experience. Keep blockers and decisions that affect that
choice visible, with links to evidence.

Preserve earlier reasoning in durable records without making readers replay every
checkpoint. Retain unresolved constraints, contrary evidence, and human decisions.
Give follow-up Tasks independently useful outcomes, rather than implementation layers.

Link related work where you explain its relevance. In prose and PR bodies, use
`[Title · Task ID or PR number](known URL)` on first mention; shorten later references
when unambiguous. State the relationship, such as builds on, supersedes, or verified by.
Use known URLs and preserve cited decisions and evidence somewhere that survives shipping.
In operational lists, put the ID first: `[Task ID or PR number · Title](known URL)`.

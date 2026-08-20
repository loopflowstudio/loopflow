---
requires: scratch/<slug>.md (elaborated design from kickoff)
produces: scratch/<slug>.md (revised to match user intent)
default_agent: claude
action_style: exploratory
---
Reshape the AI-elaborated design into the human's actual intent.

## Reviewer mode

The launch prompt identifies the reviewer for this exercise.

- **Human reviewer:** use the session below to let the human reshape the design
  and explicitly confirm its key decisions.
- **Parent reviewer:** use the Task directive, quoted user language, supplied
  evidence, and wave constraints as the best available intent. Revise the
  design, record context-backed assumptions and genuine ambiguities, and never
  wait for or invent human confirmation. Use the review protocol to ask the
  Task for missing evidence and to send each design change. Verify the Task's
  updated doc rather than editing its worktree yourself.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

Kickoff took a fuzzy task and fleshed it out. That elaboration is the AI's best guess — possibly specific but not yet shaped by the human. This session is where the human shows up and sculpts it.

## Voice

Assume the human is walking in cold. This task may have been filed by an
incident flow, another agent, or a PM sync they never read — do not assume
they know what it is, why it exists, or what has happened since. The
session's first job is context transfer: give them everything they need to
own the decisions before asking for any. A question posed before the human
has the context to answer it is a wasted question.

Come prepared, not opinionated. You've read the wave context, the task, and the kickoff output. Present your understanding and let the human reshape it. This is their design session with a knowledgeable partner, not a review they need to defend.

Don't open with evaluation ("The strongest part of this design...", "One concern is..."). Open with where they are and the wins the task is chasing. Be wrong confidently — it's faster for them to correct a clear statement than to answer open-ended questions.

## Opening

Brief first, then interpret. Four movements, in order:

0. **Where you are** — one short paragraph situating the human: what this
   task is in plain language, what filed it and why (incident, backlog,
   another agent's finding), and what happens after this session (the design
   drives implementation without further input). No jargon, no internal ids
   without explanation. Written for someone who has never seen this task.
1. **What I learned** — what the kickoff investigation found about the
   system: how the affected code actually behaves, what the real cause or
   constraint turned out to be, anything that would surprise someone who
   hasn't read the code recently. Teach it; don't reference it. This is
   where the human integrates the work so far into their own model of the
   system.
2. **The wins we're shooting for, and the key decisions** — the concrete
   improvements this task should produce, stated in plain language, then
   the choices everything else depends on. State them as decisions, not
   questions. "The design puts X in Y" — not "should X go in Y?" Make it
   easy for the human to say "yes" or "no, more like..."
3. **What feels uncertain** — places where the kickoff elaboration feels like a guess rather than an obvious conclusion. Flag these honestly, and say what context would resolve each one.

Movements 0 and 1 are the briefing; keep them tight but never skip them —
even a human who filed the task yesterday has swapped it out by now.

## Session flow

The human reshapes the design through conversation. Follow their lead.

**When they confirm** — move on. Don't linger on parts that are right.

**When they redirect** — update the design immediately. Don't accumulate changes for the end. Write to `scratch/` as you go so nothing gets lost.

**When they go deeper** — follow them. If a component needs more detail than kickoff provided, flesh it out together. This is where the human's domain knowledge meets the agent's codebase knowledge.

**When they cut scope** — respect it. Remove cleanly. Don't hedge with "we could add this later" unless they ask about sequencing.

## What to come prepared with

Read before the session starts:
- `scratch/<slug>.md` — the kickoff output
- Wave `GOAL.md`, `MEMORY.md`, and the PM snapshot's Project definition, KRs,
  and tasks — for context on where this fits
- Surrounding code in the area — so you can speak concretely about integration points
- Existing patterns and conventions — so proposals fit the codebase

Use this preparation to make concrete suggestions, not to evaluate. "The existing code does X this way, so this design could follow that pattern" — not "this design doesn't follow the existing pattern."

## Collaborative execution

During the session:
- Update the scratch doc as decisions land. Don't wait until the end.
- Sketch types and signatures when the conversation gets concrete. Code communicates faster than prose.
- If the design grows beyond one commit, say so — but let the human decide whether to split.
- If the design shrinks to something simple, say so — maybe kickoff over-elaborated and the real thing is smaller.

## End state

With a human reviewer, the scratch doc reflects explicitly confirmed intent.
With a parent reviewer, it distinguishes evidence-backed decisions from
assumptions. In either mode the doc is ready to drive implementation.

If major open questions remain, note them in the doc rather than leaving them implicit. The implementing session needs to know what's decided and what's still soft.

Make the doc comprehensive enough that the implementing agent can work from it without further input. If something feels under-specified, push the human on it now — don't leave it for implementation to guess.

## Wave alignment

If wave context is present:
- Does this design advance the wave's stated goals?
- Does it respect scope boundaries?
- Will the "done when" criteria actually move the wave forward?

Surface misalignment as information, not objection. The human may have good reasons to diverge.

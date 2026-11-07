---
requires: intended outcome and work to reconcile
produces: plan, implementation and Wave memory that agree, with a short account of progress
action_style: procedural
---
Bring the plan and the implementation into agreement with what the work has taught us.

1. Work out the intended outcome, accepted constraints and current approach from
   the conversation, review feedback, plan and code. Look at what the affected
   behavior does now. When the work belongs to a Wave, read its objective and
   `.lf/wave/<name>/MEMORY.md` alongside the plan for direction and
   lessons that should shape it. Don't attach unbound work to a Wave. At a parent
   Wave, also read its child Waves' top-level Markdown from this checkout;
   ordinary context leaves children out. When intent is missing, ask for the
   specific decision instead of treating the implementation as the spec.

2. Find what changed since the plan was written or last reconciled, here and
   elsewhere: upstream changes, related work, new decisions or lessons in shared
   memory. Could the plan fit better with what's happening around it? Take those
   improvements while keeping the intended outcome and accepted constraints.

   Then find where the work changed our understanding. Which assumptions
   failed? Which planned mechanisms turned out unnecessary? What is still
   missing, or works differently than intended? Separate what you saw from why
   you think it happened. Reuse existing results; run a focused check only when
   it settles a real question. Don't rerun tests because realign started.

3. Fix it at the source. Rewrite stale parts of the plan, delete obsolete steps,
   and update remaining work to fit what is now known. Correct small, clear
   mismatches in the code and their tests or docs. Keep the full intended
   outcome: what you learned may change the approach, but missing behavior never
   weakens acceptance. Leave a substantial code change as remaining work. When a
   change needs a product decision, write the exact choice and leave it open.
   Don't write a proposal into the plan as settled unless someone said so;
   quote what they said.

   Edit `.lf/wave/` only inside a Loopflow Graph whose step calls for it.
   Otherwise it is read-only; put proposed Wave changes in `scratch/`. The memory
   steps below apply only inside that Graph.

   Before curating the Wave's memory, list `.lf/wave/<address>/` for immediate
   child directories with a `MEMORY.md` (including ones without `GOAL.md` or a
   registry entry) and read them for lessons that apply across the parent. For
   large files, read headings and the sections that matter; go deeper when
   useful. Say what you didn't read. Promote broadly useful lessons to the
   parent and leave child-specific detail where it is. Curate the selected Wave,
   not its ancestors.

   Then update the Wave's memory: replace stale guidance, remove duplication,
   and write the direction and lessons this work produced. Quote what people
   said rather than a verdict about it, and keep contrary findings and the
   limits of what was observed. Memory carries what future Wave work should
   know, not a copy of this pass; the implementation plan stays in its own file.
   Edit `GOAL.md` and `MEMORY.md` like any other file. If memory is unavailable,
   say so and continue with the other corrections.

4. Check the result. The plan separates what exists from what remains, and the
   code backs every claim of finished behavior. Verify repairs with the smallest
   check. Keep blockers, contrary findings and deferred checks where they affect
   the next action. A check that can't run headless goes to gate or CI or gets a
   headless equivalent; don't block the Graph on it. Ask for a judgment you need
   when someone is in the conversation; headless, leave it for demo.

The plan states what will be built and what remains; quotes there show what
was said about it and may carry the speaker's name. Write down only what can't
be worked out again from the code, git history or a fresh run: what people said,
word for word, what a run showed at the time, and facts
from outside the repo. Quality per word, not volume. Leave out session
instructions and Machine facts; the plan describes the work, it doesn't direct
its next reader. Keep transcripts separate and historical skill names
unprefixed. If no plan exists, update the working context, or write the
smallest plan continued work needs.

Finish briefly: what changed, what remains, any decision needed, and one
command/result line. Update the existing plan rather than adding reports. If
everything already agrees, say so. Give loop-or-next the facts to judge; don't
preselect Advance, Iterate or Blocked. Publishing and moving the Task on belong
to the caller.

## Keep authored context within budget

Run `lf context` for memory and scratch size targets, where they are configured
and current usage. Read sources in full before curating them. Inside the Graph's
memory steps, trim Wave memory gradually: when it exceeds either limit, retire
the largest stale sections to git history until it is just under both. Stop once
it fits; don't rewrite the whole memory toward a smaller target or shrink one
that is already in budget. Keep correcting stale guidance and recording lessons
where this work changes them.

Bring over-budget scratch under both limits too: merge duplicates, summarize
long notes, drop obsolete notes inherited from a stacked parent, and leave
history in git with a precise reference when older detail still matters. Keep
the live plan, exact quotes in scratch notes, open questions
and contrary findings. Commit or move uncommitted notes before removing them.
Edit existing notes rather than adding reports. Re-run `lf context` after
writing. Never raise limits to hide overflow; if the live decisions alone don't
fit, record the conflict and the remaining overage.

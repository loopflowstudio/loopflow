---
requires: intended outcome and work to reconcile
produces: aligned plan, implementation, and Wave memory, with a minimal review of progress and evidence
default_agent: claude
action_style: procedural
---
Bring the plan and the implementation into agreement with what the work has taught us.

1. Recover the intended outcome, accepted constraints, and current approach
   from the supplied conversation, review feedback, plan, and relevant code. Read existing
   evidence and inspect the affected behavior. Use the context available;
   do not require a Task, branch, previous pass, or particular document layout.
   When the work identifies a Wave, read its objective and `wave/<name>/MEMORY.md`
   alongside the plan. Recover accepted decisions, lessons, and evidence limits
   that should shape the work. Use supplied Wave context or the repository's
   memory location; do not invent a Wave for unbound work.
   When intent is missing, ask for the specific missing decision rather than
   treating the implementation as its own specification.

2. Identify relevant changes and memories added since the plan was generated
   or last reconciled. Look beyond this implementation: upstream changes,
   related work, and new decisions or lessons in shared memory may change the
   best approach. Is there an opportunity to adjust the plan to better fit
   what is happening elsewhere? Incorporate supported improvements while
   preserving the intended outcome and accepted constraints.

   Find where the work changed our understanding. Which assumptions failed?
   Which planned mechanisms became unnecessary? What remains unimplemented,
   or works differently than intended? Separate observations from explanations.
   Reuse applicable results; run a focused check when it will resolve uncertainty.
   Do not rerun tests just because reconciliation began.

3. Reconcile at the source. Rewrite stale portions of the plan, delete obsolete
   steps and explanations, and update the remaining work to fit what is now known.
   Correct clear, bounded implementation mismatches and their affected tests or
   docs. Preserve the full intended outcome and still-relevant constraints.
   Evidence may change the approach; missing behavior does not justify weakening
   acceptance. Keep a substantial code change explicit as remaining work.
   When a change needs a product decision, show the exact choice and leave it
   unresolved. Do not silently promote a proposal into an accepted decision.

   Reconcile the Wave's memory too. Replace stale guidance, remove duplication,
   and write durable decisions and lessons learned from this work into its
   existing memory. Attribute decisions by name and preserve contrary evidence
   and limits of the observations. Keep the live implementation plan in its own artifact;
   memory carries what future Wave work should know, not a copy of the pass.
   Follow the repository's memory workflow. If identified memory is unavailable
   or cannot be updated within the supplied authority, state that exact gap
   and continue the independent local corrections.

4. Check the resulting agreement. The plan must distinguish what exists from
   what remains, and the code must support claims of completed behavior. Verify
   repairs with the smallest relevant check. Keep blockers, contrary evidence,
   and deferred checks visible where they affect the next action. If a check cannot
   run headless, leave it to gate/CI or replace it with a headless check; do not
   block the Flow on it. Human judgment belongs to demo/review.

Keep named, dated decisions, draft/accepted status and remaining work in the
plan. Record one check-result line. Omit session instructions and ambient Home
facts; the plan must not direct its next reader. Keep transcripts separate and
historical skill names unprefixed.

If no plan exists, update the supplied working context or write the smallest
useful plan when continued work needs one.

Finish briefly: what changed, what remains, and any decision needed. Keep the
check result to one line with its command and result or deferred owner. Update
the existing plan; do not accumulate reports or repeated untested-claim lists.
If everything already agrees, say so without manufacturing changes.
Supply the facts for a reader such as loop-decide to judge; do not preselect
Advance, Iterate, or Blocked. Publication and workflow navigation belong to
the caller.

## Keep authored context within budget

Use the assembled `lf:context-budget` snapshot, or run `lf context --skill realign`
to read effective limits, their configuration sources, and current usage. Before
updating scratch or Wave memory, read complete sources named by excerpt pointers.
Bring over-budget material under both token and byte limits as part of this step.
Merge duplicates, summarize long evidence, remove obsolete notes inherited from a
stacked parent, and keep historical detail in git rather than ambient context.
Preserve live decisions, attribution, unresolved work, and contrary evidence;
keep a precise git reference when older detail still matters. Preserve uncommitted
evidence before removing it. Edit existing notes instead of accumulating reports.
Re-run the query after writing. Do not raise limits to conceal overflow. If the
live decisions alone cannot fit, record the concrete conflict and remaining overage.

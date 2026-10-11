---
requires: objective, current evidence and feedback, and the caller's decision protocol
produces: Advance or Iterate with evidence, or a Blocked outcome for human resolution
action_style: procedural
---
Decide whether the Flow should advance, iterate, or stop because progress is blocked.

Use the objective, success conditions, and constraints supplied by the caller.
Do not impose a particular workflow, producer skill, artifact format, or
progress metric. Work and review steps supply domain-specific criteria and
evidence; this step owns the navigation judgment.

Read the previous direction, what changed or was learned, current feedback,
and unresolved findings. Follow supplied artifact paths and inspect their
current contents when they may have changed. Reconcile earlier evidence and
accepted changes; a newer proposal does not override an accepted requirement.
Reuse applicable results without treating a process exit as task completion.

An unavailable or human-dependent check alone never means Blocked. Use a
headless equivalent or name its later gate/CI owner and continue. Before gate,
build/focused-test sanity is enough. Actual failures and missing behavior still
need work. Leave judgment to demo/review; neither claim deferred checks passed
nor bypass an authored review.

- **Advance** when the work required at this boundary is satisfied, including
  applicable acceptance conditions and findings from earlier attempts. Leave
  later obligations to their declared steps.
- **Iterate** when meaningful progress was made and specific remaining work can
  use the declared backward edge. Supply the next action and observable result,
  within the accepted scope. Learning that rules out a hypothesis or narrows
  the problem counts as progress; activity alone does not.
- **Blocked** when the previous iteration made no meaningful progress, repeated
  the same failure without new evidence, or exposed an input or judgment the
  agent cannot supply. Honor unresolved blockers in the supplied criteria and
  evidence. Explain what needs resolving. Do not spend another iteration repeating unchanged direction.
  If evidence is missing, name that gap rather than pretending it establishes no progress.

Separate the questions before deciding: which obligations are satisfied, what
changed or was learned, and what useful action or missing human input remains.
Inspect the relevant evidence; do not substitute a confidence score for observed results.
The runner owns edge targets and execution identity. There is no pass limit;
judge whether to continue from the work and evidence, not the iteration count.

Use the supplied decision protocol for this exact occurrence. Advance and
Iterate navigate; Blocked stops execution at the current position. Explain the
missing input or repeated failure in the declared outcome. Existing output and
logs are sufficient; no separate handoff or conversation is required.

On an explicit retry, reassess using the new evidence or direction. Retry does
not establish that the work is done and does not choose navigation. Authored
reviews return feedback for this assessment, not a verdict.
Carry accepted changes, unresolved questions, and the concrete next action into
the decision summary, with references the next step can follow.

Output validation feedback requests a corrected value in this same conversation.
Return the declared verdict and evidence summary;
do not rerun implementation to repair output. Missing or malformed output does
not establish that the work made no progress. If the protocol still cannot be
satisfied, leave the specific failure visible rather than inventing success.

Without a bound decision protocol, return this assessment in the conversation.
Do not invent a decision command, create a Task, launch a successor, or write a
file for a later agent to interpret as execution authority.

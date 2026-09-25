---
requires: accepted objective, iteration evidence, review findings, and the current Flow decision protocol
produces: Advance or Iterate with evidence, or a Blocked outcome for human resolution
action_style: procedural
---
Decide whether the Flow should advance, iterate, or ask a human to resolve stalled progress.

Read the accepted objective, the previous iteration's direction, what this pass
changed or learned, and the current review findings. Reuse recorded proof when
it still applies. Work and review steps supply evidence; this decision step owns
the navigation choice. Do not require the preceding agent to write a decision
file or infer its decision from a successful process exit.

Use `scratch/` as the shared working record. Start with the exact note paths
named in the incoming feedback or direction, then read the current design and
relevant review/proof notes. Inspect the rest of scratch for unresolved findings
or accepted changes that the handoff omitted. If no paths were supplied, locate
those notes by their topic and contents. No special filename, latest-file rule,
or producer skill determines which evidence is authoritative. Reconcile dated
observations, explicit human decisions, and superseded conclusions; a newer
proposal does not replace an accepted requirement by itself. Read the current
files when they may have changed since this Run's context was assembled.

After an interactive demo, distinguish observed behavior, accepted design
changes, remaining implementation, and unresolved questions in those notes.
Completing the demo means the conversation ended; it is not a verdict that the
work is finished. Iterate to the authored target when the revised design needs
implementation. Cite the relevant note paths and the concrete next action/proof
in your decision summary so the next work step can use the same evidence.
Advance only when the current design and feedback are satisfied. Missing or
contradictory evidence is a reason to clarify through the blocked protocol,
not to infer acceptance. Scratch recommendations inform your decision; they
are not executable verdicts.

- **Advance** when the work required at this boundary is satisfied, including
  applicable whole-design claims and unresolved findings from earlier passes.
  A passing latest slice is insufficient. Leave later reviews and delivery
  to their declared steps.
- **Iterate** when meaningful progress was made and specific remaining work can
  use the declared backward edge. Supply the next action and proof, within the
  accepted scope. Learning that rules out a hypothesis or narrows the problem
  counts as progress; changed files and activity alone do not.
- **Blocked** when the previous iteration made no meaningful progress, repeated
  the same failure without new evidence, or exposed an input or judgment the
  agent cannot supply. Explain the comparison and what needs resolving. Do not
  spend another iteration repeating unchanged direction. If evidence is missing,
  name that gap rather than pretending it proves no progress.

Separate the questions before deciding: which obligations are satisfied, what
changed or was learned, and what useful action or missing human input remains.
Inspect the relevant evidence; do not substitute a confidence score for proof.
The runner owns edge targets and execution identity. There is no pass limit;
judge whether to continue from the work and evidence, not the iteration count.

Use the supplied decision protocol for this exact occurrence. Advance and
Iterate navigate; Blocked reports a stopped execution requiring an Ask. Include
the attempted direction, before/after evidence, and the question in that outcome.
The Ask runs the unblock skill with the human; unblock uses concept-review by
default and narrows to a specific question when possible. Let the runtime open
that Ask once rather than also opening a second Session yourself.

After the human completes the Ask, read its summary and the changed artifacts.
Reassess using the new evidence or direction. Completion of an Ask is not proof
that the work is done and does not choose a navigation decision. If the
same blocker remains unresolved, report that fact instead of cycling through
identical Asks automatically.

A rejected decision command is correction feedback, not a failed work pass.
Correct its reported shape or authority error within this Run when possible;
do not rerun implementation to repair output. Missing or malformed output does
not establish that the work made no progress. If the protocol still cannot be
satisfied, leave the specific failure visible rather than inventing success.

Without a bound decision protocol, return this assessment in the conversation.
Do not invent a decision command, create a Task, launch a successor, or write a
file for a later agent to interpret as execution authority.

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

- **Advance** when the work required at this boundary is satisfied, including
  applicable whole-design claims and unresolved findings from earlier passes.
  A passing latest slice is insufficient. Leave later human and delivery gates
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

Use the supplied decision protocol for this exact occurrence. Advance and
Iterate navigate; Blocked reports a stopped execution requiring an Ask. Include
the attempted direction, before/after evidence, and the question in that outcome.
The Ask runs the unblock skill with the human; unblock uses concept-review by
default and narrows to a specific question when possible. Let the runtime open
that Ask once rather than also opening a second Session yourself.

After the human completes the Ask, read its summary and the changed artifacts.
Reassess using the new evidence or direction. Completion of an Ask is not proof
that the work is done and does not approve a separate human Flow gate. If the
same blocker remains unresolved, report that fact instead of cycling through
identical Asks automatically.

Without a bound decision protocol, return this assessment in the conversation.
Do not invent a decision command, create a Task, launch a successor, or write a
file for a later agent to interpret as execution authority.

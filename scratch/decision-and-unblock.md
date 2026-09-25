# Decide and unblock — accepted handoff

The human accepted Advance/Iterate as navigation choices, Blocked as execution
status, a single decision-agent skill, and this ownership:

> unblock is the lf session that gets asked by the ask

`decide` runs after the pass's work and reviews. It compares intended progress
with observed results, keeping new knowledge distinct from code churn. A pass
that made no meaningful progress reports Blocked. `Ask` creates the human
Session; `unblock` is its skill. It uses concept-review by default, narrowing
to a specific question when the missing input is known. Human completion
returns the summary and shared artifacts to decide; no navigation or separate
Flow gate is approved by completing that Ask.

## Remaining integration

- Wire the dedicated decide occurrence after concept-review in pursue; remove
  decision ownership from the concept-review prompt at that point. Preserve the
  review-slice → concept-review order in finite flows.
- Rename the public navigation outcomes Advance/Iterate and separate Blocked
  from their enum. Existing execution facts need migration, not silent loss.
- Route a Blocked report through an Ask running unblock once. Retain the Ask
  identity through interruption/recovery and make an unresolved completion
  visible rather than automatically opening identical Asks forever.
- Supply the decision agent the previous direction and iteration evidence;
  a missing history is an explicit evidence gap, not proof of no progress.
- Verify both ordinary and Task-bound loopflows through the same decision and
  Ask behavior, with human gates retaining their own exact authority.

The new skills use only a supplied execution protocol. They do not invent a
command, write a decision file, or grant authority by their final prose. Ask
skill selection is a separately testable foundation; its existence alone does
not demonstrate automatic Blocked escalation.

## Skill review cases

| Evidence | Expected behavior |
| --- | --- |
| Behavior and whole-design claims hold | Advance; leave later gates intact |
| A concrete gap remains after useful progress | Iterate with action and proof |
| No code changed, but investigation eliminated a hypothesis | Count the learning as progress |
| Another pass repeats the same failure without new evidence | Blocked; Ask running unblock |
| Scope/model is unclear | Unblock uses concept-review with the human |
| Missing credential or one policy choice | Unblock resolves the targeted question |
| Human ends the Ask while the blocker remains | Report unresolved; no identical automatic Ask loop |
| Human ends the Ask after changing direction | Decide rereads artifacts and reassesses |

These are source-review scenarios, not live provider/Session proof.

## Bounded implementation evidence

Added the canonical decide/unblock skills and synced them, with the updated
concept-review skill, to the personal Codex skill directory. The standard Codex
metadata/body export validates for all three; Loopflow's builtin discovery test
also passes. The Ask skill-selection contribution passes its persistence/prompt
and CLI tests; see `ask-skill-proof.md`. Main additionally ran all-target Clippy,
format checks, diff whitespace, and the architecture ownership check successfully.
No live Ask, provider, Ghostty handoff, or automatic Blocked routing was exercised.

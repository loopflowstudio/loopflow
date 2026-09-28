---
requires: a working design and the intended outcome
produces: the design revised to reflect accepted intent, with open choices explicit
default_agent: claude
action_style: exploratory
---
Reshape the working design around the experience the user wants.

## Find the consequential choice

Read the supplied design, relevant conversation, accepted decisions, and enough
of the system to make concrete suggestions. Use Wave or Task context when
supplied; neither is a prerequisite. The design may be an early sketch, an
implementation plan, or a revision informed by working code.

Start where the conversation is. Give only the context the person needs to
judge the next consequential choice. Someone returning after time away may
need the problem and discoveries explained; someone already shaping the design
may need one specific alternative. Do not require a kickoff recap or a fixed
opening script.

Walk through the intended interaction and its recovery. Follow the consequences
through concepts, APIs, ownership, and proof. Make the proposed behavior tangible
in usage examples or the affected design section. Surface tradeoffs that change
the outcome, scope, or commitments; resolve routine details from evidence.

## Edit as understanding changes

Update the working design as decisions land. Rewrite mistaken assumptions and
remove rejected approaches from the current instructions. Preserve consequential
rationale, accepted constraints, and unresolved contrary evidence. Keep proposals
distinct from accepted decisions, and attribute decisions to the person who made
them. Existing code and detailed prose are evidence, not approval.

Follow the user's corrections and depth of interest. When scope changes, reconcile
the whole design so its architecture, remaining work, and proof agree. Preserve
the full target of an indivisible change while refining its internal slices.
Do not weaken acceptance to accommodate unfinished implementation.

If no person is present, improve what the supplied intent and evidence support.
Leave assumptions and the exact unresolved choices visible; do not invent
confirmation or wait for an unavailable reviewer. When the caller supplies a
review protocol or a separate artifact owner, use that path for proposed changes
and verify the resulting artifact. Otherwise edit the local working design.

## Leave a usable design

The design should make the intended outcome, chosen approach, remaining work,
and proof clear. Ask about consequential gaps while the person is present;
retain unanswered questions as questions. Briefly identify what changed and what
still needs a decision. Do not create a separate design-review report or infer
permission to launch implementation from completion of this skill.

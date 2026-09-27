# Docs voice design review

2026-09-26 · Interactive review for LOO-309 · Jack requested completion

## Context and evidence

The [current design](the-docs-read-in-the.md) carries Jack's accepted audience
and writing direction from [growth memory](../wave/growth/MEMORY.md).
The Task covers the remaining public docs and the `llms.txt` introduction.
The implementation scope is the full Task. Jack rejected the agent's proposed
Conducting-only delivery and page-by-page PR plan.

- [Conducting](../docs/conducting.md) already has an app-oriented opening,
  followed by unexplained technical language and obsolete Approve/Iterate
  instructions.
- [Session actions](../rust/loopflow/src/ops/human_session.rs) expose Complete
  for a ready Flow review: it returns feedback to the next Flow step.
- [The glossary](../docs/glossary.md) defines terms in two tables; links must
  target their section headings, not nonexistent per-term anchors.
- [The agent index generator](../website/main.py) still opens with internal
  Work/Home terminology. Its rewrite remains in the full Task scope.

## Decisions presented for review

Rewrite explanatory prose throughout each page, retain technical behavior and
reference material, explain unfamiliar terms where they occur, and extend the
shared glossary. Lead with existing Mac app capabilities and place commands
beside the relevant explanation. File-only and terminal-only operations should
say so plainly. Preserve page addresses and keep marketing out of the docs.

## Feedback and agreed changes

Jack responded to the review's “You allowed one page per PR” framing:

> No. This is totally wrong. Unbounded size per PR.

The design now has no page-count delivery limit. Its current implementation
scope includes every in-scope page, the glossary, page descriptions, overview
terminology, and `llms.txt`. Page groupings organize editing, not separate PRs.
The Conducting-only stopping point was removed from the design and assumptions.
After the agent reported no other blocking questions and retained the existing
audience, app-first approach, technical depth, glossary coverage, and
no-marketing rule, Jack replied: “ok Complete”. The corrected design is ready
to drive implementation. No new public copy has been written or approved.

## Remaining assumptions and evidence gaps

- No blocking design questions remain. Glossary coverage follows the existing
  CS 101 audience: product concepts and unfamiliar engineering terms receive
  explanations in the page itself and coverage in the shared glossary.
- No reader study or running-app walkthrough has occurred. Website checks can
  verify rendering and links; they cannot establish reader comprehension or
  prove a Mac app interaction. Rendering-tool availability remains untested
  in this review.

## Next useful action

Return this feedback to the next Flow step. The design is ready for the full
implementation and its documented checks; no page-sized delivery limit remains.
Implementation, rendering checks, and reader evidence remain unperformed.
Session completion returns feedback; this review records no Advance or Iterate
decision.

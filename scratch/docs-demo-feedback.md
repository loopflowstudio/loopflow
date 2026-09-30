# Docs voice: interactive demonstration

2026-09-26–27 · LOO-309 · Revision needed; feedback prepared for completion

Design: [The docs read in the new voice](the-docs-read-in-the.md).
Supporting evidence: [implementation review](docs-slice-evidence.md) and
[concept review](concept-review.md).

## Experience presented

The branch at `6f5cfa03a` was clean at the start of this demonstration.
Started the configured website with `uv run python dev.py serve` from
`website/`; this synchronizes the canonical docs before serving them.
The local preview is at <http://localhost:5001/docs/conducting>.

Jack was invited to read **In the Mac app**, then **Steer**, and follow a
glossary link. The review question is what still needs changing for Scott or
Kim, especially the explanation of Ready, Complete, and closing the terminal.
This is a reading demonstration of the changed docs, not a desktop walkthrough.
Jack rejected Ready/Complete as the reading test on 2026-09-27; that scenario
is superseded. The subsequent configuration comprehension exercise was also
superseded after Jack's scope and tone correction below.

## Observed by the agent

- Conducting and the glossary return HTTP 200. The served HTML contains the
  Steer anchor, the review-readiness explanation, and both glossary sections.
- `lf screenshot` captured Conducting using unattended Chromium at 1440×900.
  Inspection showed the page opening, Mac app section, glossary links, sidebar,
  and section navigation rendered without overlap in that viewport. The
  temporary capture is `/tmp/loo-309-conducting-demo.png`; it is not a durable
  repository artifact or evidence of Jack opening the page.
- Both Markdown retrieval paths return the same response. After its generated
  metadata, the body matches canonical `docs/conducting.md` exactly.
- `/llms.txt` returns HTTP 200 and opens with the accepted definition.

The first ad hoc Markdown comparison failed because it assumed the whole
response equaled the source file. Inspection of `markdown_doc_response` and
the existing retrieval test established that responses prepend metadata. The
corrected check compares the body exactly and the two complete responses to
each other; it passed. No product change was needed.

Earlier suite results remain in the implementation review; they were not
rerun for this demonstration. These local observations establish serving and
rendering, not deployed behavior, reader comprehension, desktop interactions,
or live account behavior.

## Feedback and design changes

On 2026-09-27, Jack clarified:

> the Ready/Complete interaction still needs redesigning. not a good test for docs

The agent has removed Ready/Complete from the docs demonstration criterion in
the linked design. The implementation's explanation of current behavior stays
accurate, but its source agreement does not establish that the interaction is
good. The earlier concept review's bounded finding of no blocking docs defect
does not resolve Jack's UX concern. No replacement interaction has been agreed,
and this feedback is not acceptance or rejection of the rewritten public copy.

The agent then proposed Configuration's opening and **Context Assembly** as a
reading path: add a file for one run, understand when to save that choice,
and understand why extra files are opt-in. This proposal is now superseded.
The preview at <http://localhost:5001/docs/config> returned HTTP 200 on
2026-09-27.

Jack responded:

> Hmm. this feels like speaking down to people. I think you've gone too far. our job is not to explain how ais to work together. just to explain how loopflow works

This is negative editorial feedback, not acceptance of the rewritten docs.
Jack's explicit direction is to explain Loopflow. The agent interprets this
as requiring removal of general AI collaboration coaching, repeated elementary
definitions, and the comprehension-quiz framing. The exact replacement wording
has not been approved. The design now makes this correction authoritative over
the earlier exhaustive vocabulary and teaching requirements.

Illustrative replacement proposed by the agent, not approved copy:

> Set repository defaults in `.lf/config.yaml` and personal defaults in
> `~/.lf/config.yaml`. Command-line flags override saved settings. Lists such
> as `docs` combine across configuration files.

The remaining implementation direction is recorded in the design's
**Current correction — 2026-09-27** section. It covers the full changed corpus,
preserves technical contracts, and calls for direct examples of Loopflow
operations. No public copy was changed during this demo review.

## Remaining review and next action

Recommended next work: revise the full docs prose against the corrected scope,
then present representative rendered passages for Jack's editorial judgment.
Verify affected links and Markdown retrieval after revision with the focused
docs suite from the design. Existing successful serving checks do not overcome
the editorial rejection. Reader comprehension remains unmeasured and is not
a new prerequisite for completing this review.

Ready/Complete redesign remains unresolved and separate from this docs pass.
No replacement interaction has been specified. The current docs must retain
accurate behavior without treating the interaction as accepted UX.

The review has actionable feedback and revised implementation direction, and
can be marked ready for Jack to complete. Readiness does not mean the copy
passed review. Jack owns completion; a following deciding step owns Flow
navigation. This note records no merge approval, Task completion, or navigation
decision.

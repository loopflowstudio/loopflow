# Concept review skill

Create a builtin skill for reconsidering product concepts, core data structures,
and APIs with a human mid-task, and for a bounded autonomous review after
review-slice. Product simplification is worthwhile on its own; follow it through
to the infrastructure for additional gains, not as the purpose of the review.
Preserve the user's problem and accepted constraints while making
the current design available for reconsideration.

Start by rewriting usage documentation and skills to express the proposed
experience, before code changes and potentially before implementation design.
Use the draft to discover the model; distinguish proposed from verified behavior.

Done when: the skill is discoverable, defines attended and autonomous behavior,
grounds simplification in concrete preserved experience, and all flows directly
using review-slice follow it with concept-review. In pursue, concept-review owns
the repeat verdict so completion cannot skip it. The real driver fixture must
exercise two four-step passes and saved-verdict recovery. Already pinned Flows
retain their definitions.

Historical evidence: inspect first-parent history ranked by net source deletion,
then PR descriptions and affected paths. #872, #1099, and #1237 demonstrate
concept removal deleting infrastructure; #1270 demonstrates why deletion alone
does not prove preserved experience. Keep dated evidence in docs/concept-review.md;
the portable builtin must not depend on repository-specific files.

Terminology raised by the User: Flow is the whole sequence; a loop repeats a
section; a pass/iteration traverses that section; a slice is a bounded unit of
work, potentially one stage; review judges work or its design. The runtime's
LoopReview currently also carries loop progress. This skill addition does not
rename stored runtime types or implement the earlier Advance API proposals.

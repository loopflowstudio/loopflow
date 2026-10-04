# Assumptions · LOO-367 · 2026-10-02

October 4 disposition: Jack Heart ended the review and authorized pursuing the
revised design. Follow its “Authorized revision” section. Earlier open-review
language below is historical. Resolve routine handoff/cleanup mechanics during
implementation while preserving the accepted behavior and other Tasks' scope;
return to the next authored demo with behavior evidence.

Jack Heart confirmed on 2026-10-02 that the working conversation can complete its
Task. No further product question is outstanding from this review. The
[working design](jack-heart/start-and-finish-tasks-without.md) retains the cleanup
implications and acceptance proof still required. The scope choices below are now implemented;
this records their status without claiming separate product approval.

- The kickoff design preserves existing current-Project selection for filing;
  LOO-366 owns Project availability and optional chapter resets. Narrow issue
  confirmation is in scope, while auto-creating a Project is not.
- Optional Task placement is the branch implementation choice. An attributed
  command without placement uses its caller cwd without adopting that directory;
  explicit associations carry the history. Existing delivery placement retains
  its routing and protections.
- LOO-364 retains broader Session wake and Flow switching UX. LOO-367 removes
  unrelated delivery prerequisites from shared admission and control operations,
  without adding another execution lifecycle.

## Implementation finding — October 4

Native Move here is not a preservation-safe directory handoff. It stops the
client and explicitly loses unsent input; a stored cwd change is not execution
movement. The design records the exact paths and missing Session interface.
LOO-353's returned brief is still kickoff-only pending review. The shared execution mechanism remains unresolved; production code and accepted
outcomes are unchanged. Prior-directory edits remain in place.

## Earlier product review — October 4

Jack Heart said the product is quite hard and that this is why the review has
not been reached yet. The earlier decision allowing a working conversation to
complete its Task does not establish acceptance of the whole experience.
Keep LOO-367 at product/demo review; no autonomous delivery or review waiver was
requested. Discuss the Task/Session/Flow experience before treating the remaining
work as merely demonstration setup. Preserve the existing implementation as
material for that discussion, not as a settled product contract.

Jack Heart subsequently expressed a tentative preference to default the transition
from an open-ended Session to a focused Task to having a worktree, in case edits
begin. This supersedes treating no-checkout association as the preferred default.
The conversation handoff and preservation of any pre-existing edits remain open;
the earlier “no further product question” statement applies only to the October 2
completion decision. See the design's October 4 direction and
[review feedback](task-purpose-demo.md).

Jack also explicitly required clearing the associated worktree on Task completion
even when nothing landed. Empty speculative placement cannot require publication,
merge, or manual PR abandonment. The remaining design question is how a continuing
conversation leaves that checkout safely, while real unfinished work stays
protected. The current first-allocation/merged-PR fixture does not prove this path.

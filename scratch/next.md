# Slice 1: remove implicit PR review state

## Implement

- Delete `ReviewGateState` and every requested/active/approved/change-requested
  branch.
- Rename `TaskAction::Review` to presentational `OpenPr`.
- Rename `AfterMerge::Review` to `ContinueTask`.
- Add the next migration mapping stored `review` to `continue_task` and tighten
  the current check to `continue_task|complete_task`.
- Make a merged `ContinueTask` PR proceed through serial Task continuation with
  no approval record.
- Remove any dead directive or review state exposed only by the deleted gate.
- Update Rust/Swift/JSON/help/docs/tests with no compatibility alias.

## Done when

- [ ] exact old-symbol search is zero in current source;
- [ ] `OpenPr` contributes to no WorkStatus, Run, Wait, completion, or Feedback;
- [ ] current schema accepts only the two honest dispositions;
- [ ] migration and merged-continuation tests pass;
- [ ] fmt and all-target clippy pass.

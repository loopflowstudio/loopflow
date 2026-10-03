# Assumptions · LOO-367 · 2026-10-02

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

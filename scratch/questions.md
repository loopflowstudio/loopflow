# Open questions

- `put_project` still refuses a Project whose facts differ at one `updatedAt`
  (kept for the name-cutover exception). If Linear re-renders a Project field
  without a new revision, `lf wave sync` wedges the same way Tasks did; the
  message now says to edit the Project in Linear (which bumps `updatedAt`).
  Applying the later-read rule there would retire the cutover table's guard.
- "Project … is missing from its Initiative" has no lf-side remedy: no
  production path removes a Wave's Project membership. The message names the
  Linear-side fix (add it back) and `lf wave sync`.
